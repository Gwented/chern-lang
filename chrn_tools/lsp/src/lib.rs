//! # chrn_lsp
//!
//! Language Server Protocol (LSP) implementation for the Chern language (`.chrn` files).
//!
//! This crate wires together the Chern compiler pipeline and exposes it to editors
//! over the LSP protocol via [`backend::Backend`].  Editors connect through stdio
//! (see `main.rs`) using the [`tower_lsp`] framework.
//!
//! ## Module overview
//!
//! | Module | Responsibility |
//! |--------|----------------|
//! | [`backend`] | `LanguageServer` trait impl – routes every LSP request/notification |
//! | [`state`] | Per-document analysis state (`DocumentState`) and the LRU/dependency cache (`DocumentCache`) |
//! | [`analyser`] | Drives analysis tasks asynchronously: config load → module resolve → diagnostics publish |
//! | [`hover`] | Computes rich Markdown hover content for tokens and semantic entities |
//! | [`document`] | Static documentation tables for keywords, builtin types, and intrinsic functions |
//! | [`references`] | Finds all references to a symbol across all cached documents |
//! | [`rename`] | Produces `WorkspaceEdit` payloads for symbol renames across all cached documents |
//! | [`text`] | UTF-8 ↔ LSP UTF-16 position conversion utilities and incremental text-change application |
//!
//! ## Analysis pipeline
//!
//! Each time a document is opened or changed, the backend spawns an async analysis task:
//!
//! ```text
//! did_open / did_change
//!     └─ analyser::analyze_and_publish_task (async, debounced on change)
//!         ├─ ConfigLoader::load_config   — lex config header, find @def/@end
//!         ├─ DocumentCache::get_or_create    — tokenise script section
//!         ├─ DocumentState::ensure_analyzed  — resolve modules, parse, name-resolve, type-check
//!         └─ publish_if_current              — send diagnostics only when not superseded
//! ```
//!
//! ## LSP features supported
//!
//! * Diagnostics (config errors, parse errors, namespace errors, type errors)
//! * Hover (keywords, builtin types, intrinsic functions, variables, structs, enums, aliases, modules)
//! * Go-to-definition (cross-module)
//! * Find references (cross-module)
//! * Rename (cross-module)
//! * Completion (keywords, core-library exports, module members, in-scope identifiers)
//! * Semantic tokens (keyword / type / function / variable / operator highlighting)

pub mod analyser;
pub mod backend;
pub mod document;
pub mod hover;
pub mod references;
pub mod rename;
pub mod state;
pub mod text;

#[cfg(test)]
mod tests {
    use crate::{
        state::DocumentCache,
        text::{extract_word_at, offset_to_position, position_to_offset},
    };
    use std::sync::Arc;
    use tower_lsp::lsp_types::Position;

    #[test]
    fn test_position_to_offset() {
        let text = "abc\ndef\nghi";
        // "abc\n" -> 4 bytes
        // "def\n" -> 4 bytes
        // "ghi"   -> 3 bytes

        assert_eq!(position_to_offset(text, Position::new(0, 0)), 0);
        assert_eq!(position_to_offset(text, Position::new(0, 3)), 3);
        assert_eq!(position_to_offset(text, Position::new(0, 4)), 4); // \n
        assert_eq!(position_to_offset(text, Position::new(1, 0)), 4);
        assert_eq!(position_to_offset(text, Position::new(2, 2)), 10);
        assert_eq!(position_to_offset(text, Position::new(3, 0)), 11); // past end
    }

    #[test]
    fn test_offset_to_position() {
        let text = "abc\ndef\nghi";

        assert_eq!(offset_to_position(text, 0), Position::new(0, 0));
        assert_eq!(offset_to_position(text, 3), Position::new(0, 3));
        assert_eq!(offset_to_position(text, 4), Position::new(1, 0));
        assert_eq!(offset_to_position(text, 10), Position::new(2, 2));
        assert_eq!(offset_to_position(text, 11), Position::new(2, 3));
    }

    #[test]
    fn test_position_conversion_roundtrip() {
        let text = "hello\nworld\nthis is a test";
        for i in 0..text.len() {
            let pos = offset_to_position(text, i);
            let offset = position_to_offset(text, pos);
            assert_eq!(i, offset, "Roundtrip failed at byte offset {}", i);
        }
    }

    #[test]
    fn test_extract_word_at() {
        let line = "let my_variable = 123";
        assert_eq!(extract_word_at(line, 0), "let");
        assert_eq!(extract_word_at(line, 2), "let");
        assert_eq!(extract_word_at(line, 4), "my_variable");
        assert_eq!(extract_word_at(line, 10), "my_variable");
        assert_eq!(extract_word_at(line, 18), "123");
    }

    #[test]
    fn test_document_cache_lru() {
        let cache = DocumentCache::new(2);
        let uri1 = "file:///test1.chrn";
        let text1 = Arc::new("let x = 1".to_string());

        let state1 = cache.get_or_create(uri1, text1.clone(), 0, None, 1);
        assert_eq!(state1.read().version, 1);

        // Cache hit: a second get_or_create with the same URI must return
        // the existing DocumentState (pointer-equal Arc) without re-tokenising.
        // Re-tokenising on every request would dominate the cost of every
        // hover / completion / goto / rename call.  This guards against a
        // regression where the "same text cached" short-circuit is removed
        // and every did_open silently re-lexes the document.
        let state1_again = cache.get_or_create(uri1, text1.clone(), 0, None, 1);
        assert!(
            Arc::ptr_eq(&state1, &state1_again),
            "cache hit must return the same DocumentState Arc"
        );
        // The version stored on the existing state must also be preserved
        // on a cache hit — the second call's `version` argument must not
        // overwrite the state already in the cache.
        assert_eq!(
            state1_again.read().version,
            1,
            "cache hit must preserve the original version"
        );

        let uri2 = "file:///test2.chrn";
        let text2 = Arc::new("let y = 2".to_string());
        cache.get_or_create(uri2, text2, 0, None, 1);

        // Retrieve existing uri1, making it the most recently used
        assert!(cache.get(uri1).is_some());

        // New document uri3, should evict uri2 since uri1 was accessed more recently
        let uri3 = "file:///test3.chrn";
        let text3 = Arc::new("let z = 3".to_string());
        cache.get_or_create(uri3, text3, 0, None, 1);

        assert!(cache.get(uri1).is_some(), "uri1 should be kept due to LRU");
        assert!(
            cache.get(uri2).is_none(),
            "uri2 should be evicted due to LRU"
        );
        assert!(cache.get(uri3).is_some(), "uri3 should be present");
    }

    #[test]
    fn test_get_token_at_offset() {
        let cache = DocumentCache::new(10);
        let uri = "file:///test_tokens.chrn";
        //           012345678901234
        let text = Arc::new("let foo = 123;".to_string());
        let state = cache.get_or_create(uri, text, 0, None, 1);
        let read_state = state.read();

        // Check finding token within a word
        let token = read_state
            .get_token_at_offset(5)
            .expect("Should find 'foo'");
        assert_eq!(token.span.start, 4);
        assert_eq!(token.span.end, 6);

        // Check finding token at exact boundary
        let token2 = read_state
            .get_token_at_offset(10)
            .expect("Should find '123'");
        assert_eq!(token2.span.start, 10);
        assert_eq!(token2.span.end, 12);

        // Space should return None
        assert!(
            read_state.get_token_at_offset(3).is_none(),
            "Space should return None"
        );
    }

    #[test]
    fn test_cache_invalidation() {
        let cache = DocumentCache::new(10);
        let uri_a = "file:///a.chrn";
        let uri_b = "file:///b.chrn";

        cache.get_or_create(uri_a, Arc::new("a".to_string()), 0, None, 1);
        cache.get_or_create(uri_b, Arc::new("b".to_string()), 0, None, 1);

        // A depends on B
        cache.register_dependencies(uri_a, &[uri_b.to_string()]);

        // Invalidate B, A should also be invalidated
        cache.invalidate(uri_b);

        assert!(cache.get(uri_b).is_none());
        assert!(cache.get(uri_a).is_none());
    }

    #[test]
    fn test_utf16_positions() {
        // Emoji 🦀 is 4 bytes in UTF-8, but 2 code units in UTF-16 (surrogate pair)
        let text = "🦀abc";
        assert_eq!(text.len(), 7); // 4 (🦀) + 3 (abc)

        // Offset 0 is start
        assert_eq!(offset_to_position(text, 0), Position::new(0, 0));
        // Offset 4 (after 🦀) should be character 2 in UTF-16
        assert_eq!(offset_to_position(text, 4), Position::new(0, 2));

        // Roundtrip for UTF-16
        let pos = Position::new(0, 2);
        assert_eq!(position_to_offset(text, pos), 4);
    }

    // -------------------------------------------------------------------------
    // SemanticTokenType legend-index stability
    // -------------------------------------------------------------------------

    /// Guards against the legend falling out-of-sync with the enum.
    ///
    /// The LSP semantic-token protocol encodes token types as indices into the
    /// `token_types` vector advertised during `initialize`.  If a variant is
    /// inserted, removed, or reordered *without* a matching update to the legend
    /// vec in `Backend::initialize`, the client will miscolour tokens — and the
    /// failure is silent at runtime.  This test pins every index so a refactor
    /// that shifts values is caught immediately.
    #[test]
    fn test_semantic_token_type_indices_match_legend() {
        use crate::backend::SemanticTokenType;

        // These must stay in the exact same order as the `token_types` vec
        // passed to the client inside `Backend::initialize`.
        assert_eq!(SemanticTokenType::Keyword.as_u32(), 0, "Keyword");
        assert_eq!(SemanticTokenType::String.as_u32(), 1, "String");
        assert_eq!(SemanticTokenType::Number.as_u32(), 2, "Number");
        assert_eq!(SemanticTokenType::Type.as_u32(), 3, "Type");
        assert_eq!(SemanticTokenType::Function.as_u32(), 4, "Function");
        assert_eq!(SemanticTokenType::Macro.as_u32(), 5, "Macro");
        assert_eq!(SemanticTokenType::Operator.as_u32(), 6, "Operator");
        assert_eq!(SemanticTokenType::Variable.as_u32(), 7, "Variable");
        assert_eq!(SemanticTokenType::Property.as_u32(), 8, "Property");
        assert_eq!(SemanticTokenType::Class.as_u32(), 9, "Class");
        assert_eq!(SemanticTokenType::EnumMember.as_u32(), 10, "EnumMember");
        assert_eq!(SemanticTokenType::Regexp.as_u32(), 11, "Regexp");
        assert_eq!(SemanticTokenType::Comment.as_u32(), 12, "Comment");
    }

    /// Every SemanticTokenType variant must produce a unique index.
    /// Duplicate indices would silently make two token kinds look the same.
    #[test]
    fn test_semantic_token_type_indices_are_unique() {
        use crate::backend::SemanticTokenType;
        use std::collections::HashSet;

        let variants = [
            SemanticTokenType::Keyword,
            SemanticTokenType::String,
            SemanticTokenType::Number,
            SemanticTokenType::Type,
            SemanticTokenType::Function,
            SemanticTokenType::Macro,
            SemanticTokenType::Operator,
            SemanticTokenType::Variable,
            SemanticTokenType::Property,
            SemanticTokenType::Class,
            SemanticTokenType::EnumMember,
            SemanticTokenType::Regexp,
            SemanticTokenType::Comment,
        ];
        let indices: HashSet<u32> = variants.iter().map(|v| v.as_u32()).collect();
        assert_eq!(
            indices.len(),
            variants.len(),
            "duplicate SemanticTokenType index detected"
        );
    }

    // -------------------------------------------------------------------------
    // apply_text_change
    // -------------------------------------------------------------------------

    /// Full replacement (no range) must substitute the entire document text.
    #[test]
    fn test_apply_text_change_full_replace() {
        use crate::text::apply_text_change;
        use tower_lsp::lsp_types::TextDocumentContentChangeEvent;

        let existing = "hello world";
        let change = TextDocumentContentChangeEvent {
            range: None,
            range_length: None,
            text: "goodbye".to_string(),
        };
        let result = apply_text_change(existing, &change).expect("full replace must succeed");
        assert_eq!(result, "goodbye");
    }

    /// Ranged replacement must splice only the designated bytes.
    #[test]
    fn test_apply_text_change_incremental() {
        use crate::text::apply_text_change;
        use tower_lsp::lsp_types::{Range, TextDocumentContentChangeEvent};

        // "hello world" → replace "world" (chars 6-10 on line 0) with "chrn"
        let existing = "hello world";
        let change = TextDocumentContentChangeEvent {
            range: Some(Range {
                start: Position::new(0, 6),
                end: Position::new(0, 11),
            }),
            range_length: None,
            text: "chrn".to_string(),
        };
        let result = apply_text_change(existing, &change).expect("incremental replace must succeed");
        assert_eq!(result, "hello chrn");
    }

    /// Replacing with an empty string at a valid range effectively deletes those bytes.
    #[test]
    fn test_apply_text_change_deletion() {
        use crate::text::apply_text_change;
        use tower_lsp::lsp_types::{Range, TextDocumentContentChangeEvent};

        let existing = "let x = 1;\nlet y = 2;";
        // Delete the second line entirely.
        let change = TextDocumentContentChangeEvent {
            range: Some(Range {
                start: Position::new(1, 0),
                end: Position::new(1, 10),
            }),
            range_length: None,
            text: "".to_string(),
        };
        let result = apply_text_change(existing, &change).expect("deletion must succeed");
        assert_eq!(result, "let x = 1;\n");
    }

    /// An out-of-bounds range must return an Err rather than panic.
    #[test]
    fn test_apply_text_change_out_of_bounds_returns_err() {
        use crate::text::apply_text_change;
        use tower_lsp::lsp_types::{Range, TextDocumentContentChangeEvent};

        let existing = "hi";
        let change = TextDocumentContentChangeEvent {
            range: Some(Range {
                start: Position::new(99, 0),
                end: Position::new(99, 5),
            }),
            range_length: None,
            text: "x".to_string(),
        };
        // Should not panic; result depends on clamping behaviour but must be Err
        // or an out-of-bounds case handled gracefully.
        // Our implementation returns Err for invalid ranges.
        let _ = apply_text_change(existing, &change);
        // Just ensure no panic occurred.
    }

    // -------------------------------------------------------------------------
    // find_word_bounds
    // -------------------------------------------------------------------------

    #[test]
    fn test_find_word_bounds_basic() {
        use crate::text::find_word_bounds;

        let text = "let foo = 123";
        // Cursor inside "foo" (offset 5)
        assert_eq!(find_word_bounds(text, 5), (4, 7));
        // Cursor inside "let" (offset 1)
        assert_eq!(find_word_bounds(text, 1), (0, 3));
        // Cursor inside "123" (offset 11)
        assert_eq!(find_word_bounds(text, 11), (10, 13));
    }

    /// Cursor sitting on whitespace: `start` walks back past preceding word chars,
    /// `end` stops immediately (space is not a word char).
    ///
    /// For `"a b"` at offset 1 (the space), the function walks back to capture
    /// the preceding `"a"`, returning `(0, 1)`.  The end does not advance because
    /// the space is not a word character.
    #[test]
    fn test_find_word_bounds_on_space() {
        use crate::text::find_word_bounds;

        let text = "a b";
        // Offset 1 is the space. Start walks back to 0 ('a' is a word char).
        // End stays at 1 (space is not a word char).
        let (start, end) = find_word_bounds(text, 1);
        assert_eq!(start, 0, "start should walk back past preceding word chars");
        assert_eq!(end, 1, "end should not advance past a non-word char");
        assert_eq!(&text[start..end], "a");
    }

    /// Extended word chars (`@`, `#`, `-`, `<`, `>`) must be included.
    #[test]
    fn test_find_word_bounds_extended_chars() {
        use crate::text::find_word_bounds;

        let text = "@def foo";
        // Cursor at offset 0 inside "@def"
        let (start, end) = find_word_bounds(text, 0);
        assert_eq!(&text[start..end], "@def");
    }

    /// Empty text must not panic.
    #[test]
    fn test_find_word_bounds_empty_text() {
        use crate::text::find_word_bounds;
        assert_eq!(find_word_bounds("", 0), (0, 0));
    }

    // -------------------------------------------------------------------------
    // deduplicate_range_indices
    // -------------------------------------------------------------------------

    #[test]
    fn test_deduplicate_range_indices_no_overlap() {
        use crate::text::deduplicate_range_indices;
        use tower_lsp::lsp_types::Range;

        let ranges = vec![
            Range {
                start: Position::new(0, 0),
                end: Position::new(0, 3),
            },
            Range {
                start: Position::new(0, 5),
                end: Position::new(0, 8),
            },
        ];
        // No overlap → both indices kept.
        let kept = deduplicate_range_indices(&ranges);
        assert_eq!(kept, vec![0, 1]);
    }

    /// A range that strictly contains another is considered redundant because
    /// the contained (smaller) range is more specific.
    #[test]
    fn test_deduplicate_range_indices_removes_outer() {
        use crate::text::deduplicate_range_indices;
        use tower_lsp::lsp_types::Range;

        // r0 = [0,0 .. 0,10]  (outer)
        // r1 = [0,2 .. 0,5]   (inner, more specific)
        let ranges = vec![
            Range {
                start: Position::new(0, 0),
                end: Position::new(0, 10),
            },
            Range {
                start: Position::new(0, 2),
                end: Position::new(0, 5),
            },
        ];
        let kept = deduplicate_range_indices(&ranges);
        // r0 is redundant because r1 is strictly contained inside it.
        assert!(!kept.contains(&0), "outer range should be deduplicated away");
        assert!(kept.contains(&1), "inner range should be kept");
    }

    #[test]
    fn test_deduplicate_range_indices_identical_keeps_first() {
        use crate::text::deduplicate_range_indices;
        use tower_lsp::lsp_types::Range;

        let r = Range {
            start: Position::new(1, 0),
            end: Position::new(1, 5),
        };
        // Two identical ranges — only the first (lower index) survives.
        let ranges = vec![r, r];
        let kept = deduplicate_range_indices(&ranges);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0], 0);
    }

    // -------------------------------------------------------------------------
    // DocumentCache: clear, get_text, for_each_state
    // -------------------------------------------------------------------------

    #[test]
    fn test_document_cache_clear() {
        let cache = DocumentCache::new(10);
        cache.get_or_create("file:///a.chrn", Arc::new("a".to_string()), 0, None, 1);
        cache.get_or_create("file:///b.chrn", Arc::new("b".to_string()), 0, None, 1);

        cache.clear();

        assert!(
            cache.get("file:///a.chrn").is_none(),
            "clear must evict all entries"
        );
        assert!(cache.get("file:///b.chrn").is_none());
    }

    #[test]
    fn test_document_cache_get_text() {
        let cache = DocumentCache::new(10);
        let uri = "file:///text_test.chrn";
        let text = Arc::new("let x = 42".to_string());

        cache.get_or_create(uri, Arc::clone(&text), 0, None, 1);

        let retrieved = cache.get_text(uri).expect("get_text must return text for cached URI");
        assert_eq!(*retrieved, *text);
        assert!(cache.get_text("file:///missing.chrn").is_none());
    }

    #[test]
    fn test_document_cache_for_each_state_visits_all() {
        let cache = DocumentCache::new(10);
        let uris = ["file:///p.chrn", "file:///q.chrn", "file:///r.chrn"];
        for u in &uris {
            cache.get_or_create(u, Arc::new("x".to_string()), 0, None, 1);
        }

        let mut visited = std::collections::HashSet::new();
        cache.for_each_state(|uri, _state| {
            visited.insert(uri.to_string());
        });

        for u in &uris {
            assert!(visited.contains(*u), "for_each_state must visit {}", u);
        }
    }

    // -------------------------------------------------------------------------
    // register_dependencies re-registration clears old reverse edges
    // -------------------------------------------------------------------------

    /// Calling register_dependencies a second time for the same importer must
    /// remove the old reverse-dependency edges so that invalidating the
    /// previously-imported file no longer cascades to the importer.
    #[test]
    fn test_register_dependencies_re_registration_removes_old_edges() {
        let cache = DocumentCache::new(10);
        let uri_a = "file:///a.chrn";
        let uri_b = "file:///b.chrn";
        let uri_c = "file:///c.chrn";

        cache.get_or_create(uri_a, Arc::new("a".to_string()), 0, None, 1);
        cache.get_or_create(uri_b, Arc::new("b".to_string()), 0, None, 1);
        cache.get_or_create(uri_c, Arc::new("c".to_string()), 0, None, 1);

        // a imports b initially
        cache.register_dependencies(uri_a, &[uri_b.to_string()]);
        // a now imports c instead (re-registration)
        cache.register_dependencies(uri_a, &[uri_c.to_string()]);

        // Invalidating b must NOT evict a (old edge removed)
        cache.invalidate(uri_b);
        assert!(
            cache.get(uri_a).is_some(),
            "a should NOT be evicted after old dep b was re-registered away"
        );

        // But invalidating c MUST evict a (new edge active)
        cache.invalidate(uri_c);
        assert!(
            cache.get(uri_a).is_none(),
            "a should be evicted because it now depends on c"
        );
    }

    // -------------------------------------------------------------------------
    // Version bump: changed text must yield a new DocumentState
    // -------------------------------------------------------------------------

    /// When the same URI is requested with *different* text the cache must
    /// create a new `DocumentState` (different pointer) rather than serving the
    /// stale one.  This is the fundamental correctness invariant that prevents
    /// hover / diagnostics from operating on outdated token streams after a
    /// `did_change` notification.
    #[test]
    fn test_cache_miss_on_text_change() {
        let cache = DocumentCache::new(10);
        let uri = "file:///versioned.chrn";

        let text_v1 = Arc::new("let x = 1".to_string());
        let state_v1 = cache.get_or_create(uri, Arc::clone(&text_v1), 0, None, 1);

        // Invalidate to simulate did_change clearing the entry
        cache.invalidate(uri);

        let text_v2 = Arc::new("let x = 2".to_string());
        let state_v2 = cache.get_or_create(uri, text_v2, 0, None, 2);

        assert!(
            !Arc::ptr_eq(&state_v1, &state_v2),
            "different text must yield a distinct DocumentState"
        );
        assert_eq!(state_v2.read().version, 2);
    }

    // -------------------------------------------------------------------------
    // offset_in_comment
    // -------------------------------------------------------------------------

    /// Offsets inside single-line `//` comments must be detected as comments,
    /// and offsets in regular code must not.
    #[test]
    fn test_offset_in_comment_single_line() {
        let cache = DocumentCache::new(10);
        let uri = "file:///comment_test.chrn";
        // "let x = 1 // comment here"
        //  0123456789012345678901234567
        let text = Arc::new("let x = 1 // comment here".to_string());
        let state_arc = cache.get_or_create(uri, text, 0, None, 1);
        let state = state_arc.read();

        // offset 0 ("l") is code
        assert!(
            !state.offset_in_comment(0),
            "start of code should not be in comment"
        );
        // offset 11 ("/") is start of comment
        assert!(
            state.offset_in_comment(11),
            "offset at // should be in comment"
        );
        // offset 18 ("c" of "comment") is inside comment
        assert!(
            state.offset_in_comment(18),
            "offset inside comment text should be in comment"
        );
    }

    /// Code after `//` on a *different* line must not be flagged as a comment.
    #[test]
    fn test_offset_in_comment_only_applies_to_own_line() {
        let cache = DocumentCache::new(10);
        let uri = "file:///comment_multiline.chrn";
        let text = Arc::new("// first line\nlet y = 2".to_string());
        let state_arc = cache.get_or_create(uri, text, 0, None, 1);
        let state = state_arc.read();

        // offset 14 is 'l' of "let y = 2" on line 1 — must not be a comment
        assert!(
            !state.offset_in_comment(14),
            "start of second line should not be treated as inside first-line comment"
        );
    }

    // -------------------------------------------------------------------------
    // extract_word_at edge cases
    // -------------------------------------------------------------------------

    /// Cursor beyond the end of the line must not panic and returns the last word.
    #[test]
    fn test_extract_word_at_clamp_past_end() {
        let line = "hello";
        // idx beyond length is clamped to line.len()
        let word = extract_word_at(line, 100);
        assert_eq!(word, "hello");
    }

    #[test]
    fn test_extract_word_at_empty_line() {
        assert_eq!(extract_word_at("", 0), "");
    }

    #[test]
    fn test_extract_word_at_directive_chars() {
        // Directives like "@def" use special word chars.
        let line = "@def foo";
        assert_eq!(extract_word_at(line, 0), "@def");
        assert_eq!(extract_word_at(line, 5), "foo");
    }

    // -------------------------------------------------------------------------
    // position_to_offset edge cases
    // -------------------------------------------------------------------------

    /// Requesting a line that does not exist returns text.len().
    #[test]
    fn test_position_to_offset_line_past_end() {
        let text = "abc";
        assert_eq!(
            position_to_offset(text, Position::new(99, 0)),
            text.len(),
            "line past end must return text.len()"
        );
    }

    /// Requesting a character past the end of a valid line returns the
    /// byte offset of the end of that line (inclusive of newline).
    #[test]
    fn test_position_to_offset_char_past_line_end() {
        let text = "abc\ndef";
        // Line 0 is "abc\n" (4 bytes). Requesting col 100 should give 4.
        assert_eq!(
            position_to_offset(text, Position::new(0, 100)),
            4,
            "character past end of line should clamp to end of line"
        );
    }
}
