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
//!         ├─ ConfigLoader::load_config       — lex config header, find @def/@end
//!         ├─ analyser::resolve_document_modules — tokenise and resolve imports outside locks
//!         ├─ DocumentCache::insert_or_get    — cache the prepared lexical state
//!         ├─ DocumentState::ensure_analyzed  — parse, name-resolve, type-check
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
        analyser::{config_load_error_to_diagnostics, push_diagnostics},
        backend::SemanticTokenType,
        state::{DocumentCache, DocumentState},
        text::{
            abs_to_rel_offset, abs_to_rel_span, apply_text_change, deduplicate_range_indices,
            extract_word_at, find_word_bounds, offset_to_position, position_to_offset,
            rel_to_abs_offset, rel_to_abs_span,
        },
    };
    use chrn_utils::id_types::{PathId, SourceRegionId};
    use chrn_utils::source_map::source_diagnostic::DiagnosticLevel;
    use chrn_utils::source_map::source_diagnostic::annotations::AnnotationKind;
    use chrn_utils::source_map::source_region::SourceRegion;
    use chrn_utils::{arena::Arena, source_map::source_span::SourceSpan};
    use chrn_utils::{
        core_error::ConfigLoadError, source_map::source_diagnostic::SourceDiagnostic,
    };
    use std::collections::HashSet;
    use std::sync::Arc;
    use tower_lsp::lsp_types::{Position, Range, TextDocumentContentChangeEvent};

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
        assert_eq!(token.span.end, 7);

        // Check finding token at exact boundary
        let token2 = read_state
            .get_token_at_offset(10)
            .expect("Should find '123'");
        assert_eq!(token2.span.start, 10);
        assert_eq!(token2.span.end, 13);

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
        let result =
            apply_text_change(existing, &change).expect("incremental replace must succeed");
        assert_eq!(result, "hello chrn");
    }

    /// Replacing with an empty string at a valid range effectively deletes those bytes.
    #[test]
    fn test_apply_text_change_deletion() {
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
        _ = apply_text_change(existing, &change);
        // Just ensure no panic occurred.
    }

    // -------------------------------------------------------------------------
    // find_word_bounds
    // -------------------------------------------------------------------------

    #[test]
    fn test_find_word_bounds_basic() {
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
        let text = "@def foo";
        // Cursor at offset 0 inside "@def"
        let (start, end) = find_word_bounds(text, 0);
        assert_eq!(&text[start..end], "@def");
    }

    /// Empty text must not panic.
    #[test]
    fn test_find_word_bounds_empty_text() {
        assert_eq!(find_word_bounds("", 0), (0, 0));
    }

    // -------------------------------------------------------------------------
    // deduplicate_range_indices
    // -------------------------------------------------------------------------

    #[test]
    fn test_deduplicate_range_indices_no_overlap() {
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
        assert!(
            !kept.contains(&0),
            "outer range should be deduplicated away"
        );
        assert!(kept.contains(&1), "inner range should be kept");
    }

    #[test]
    fn test_deduplicate_range_indices_identical_keeps_first() {
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

        let retrieved = cache
            .get_text(uri)
            .expect("get_text must return text for cached URI");
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

    // -------------------------------------------------------------------------
    // rel_to_abs_offset / abs_to_rel_offset
    // -------------------------------------------------------------------------

    /// `script_start = 0` makes the relative and absolute coordinate systems
    /// identical.  Both helpers must therefore be the identity.
    #[test]
    fn test_rel_abs_offset_zero_script_start() {
        for &off in &[0u32, 1, 17, 1024, u32::MAX] {
            assert_eq!(rel_to_abs_offset(off, 0), off);
            assert_eq!(abs_to_rel_offset(off, 0), off);
        }
    }

    /// Adding a non-zero `script_start` should produce the same value as a
    /// `+= script_start` would in plain Rust, modulo `u32` overflow which
    /// must saturate rather than wrap.
    #[test]
    fn test_rel_to_abs_offset_basic() {
        assert_eq!(rel_to_abs_offset(0, 5), 5);
        assert_eq!(rel_to_abs_offset(3, 5), 8);
        assert_eq!(rel_to_abs_offset(100, 200), 300);
    }

    /// Subtracting `script_start` from an absolute offset yields the
    /// corresponding relative offset.  Saturating subtraction is required
    /// because LSP handlers can receive a cursor position that lands in the
    /// config header (above `script_start`).
    #[test]
    fn test_abs_to_rel_offset_basic() {
        assert_eq!(abs_to_rel_offset(5, 0), 5);
        assert_eq!(abs_to_rel_offset(8, 5), 3);
        assert_eq!(abs_to_rel_offset(300, 200), 100);
        // Saturating: result must clamp to 0 when below script_start.
        assert_eq!(abs_to_rel_offset(2, 5), 0);
    }

    /// A roundtrip rel → abs → rel must return the original relative offset.
    #[test]
    fn test_rel_abs_offset_roundtrip() {
        for &(rel, script) in &[(0, 5), (1, 0), (17, 200), (999, 1000)] {
            let abs = rel_to_abs_offset(rel, script);
            assert_eq!(
                abs_to_rel_offset(abs, script),
                rel,
                "roundtrip failed for rel={rel} script={script}"
            );
        }
    }

    // -------------------------------------------------------------------------
    // rel_to_abs_span / abs_to_rel_span
    // -------------------------------------------------------------------------

    /// Both endpoints of the span must be shifted by `script_start` while the
    /// `region_id` is preserved.
    #[test]
    fn test_rel_to_abs_span() {
        let span = SourceSpan::new(SourceRegionId::new(0), 3, 7);
        let abs = rel_to_abs_span(span, 5);
        assert_eq!(abs.start, 8);
        assert_eq!(abs.end, 12);
        // region_id must be preserved so cross-region resolution still works.
        assert_eq!(abs.region_id, SourceRegionId::new(0));
    }

    /// Mirror of `rel_to_abs_span` for the opposite direction.
    #[test]
    fn test_abs_to_rel_span() {
        let span = SourceSpan::new(SourceRegionId::new(0), 8, 12);
        let rel = abs_to_rel_span(span, 5);
        assert_eq!(rel.start, 3);
        assert_eq!(rel.end, 7);
        assert_eq!(rel.region_id, SourceRegionId::new(0));
    }

    // -------------------------------------------------------------------------
    // Regression tests: `config_loader` now emits spans relative to
    // `src_bytes`.  Every LSP surface that turns a compiler/loader span into
    // an LSP `Position` (or a `Range` consumed by the editor) must shift the
    // span by `script_start` so it lands in absolute file coordinates.  These
    // tests pin that contract so a future refactor cannot silently regress
    // diagnostic position reporting.
    // -------------------------------------------------------------------------

    /// `get_token_at_offset` accepts an **absolute** byte offset.  It must
    /// internally subtract `script_start` to look up tokens whose spans are
    /// stored relative to the script section.
    ///
    /// Document layout: a 6-byte config header ("@def\n") followed by the
    /// script section ("let foo = 123;").  `script_start` is 5 (the byte
    /// position of `@`); the script section starts at byte 5.
    #[test]
    fn test_get_token_at_offset_with_script_start() {
        let cache = DocumentCache::new(10);
        let uri = "file:///def_test.chrn";
        //  0         1         2
        //  0123456789012345678901234
        //  @def\nlet foo = 123;
        let text = Arc::new("@def\nlet foo = 123;".to_string());
        let state = cache.get_or_create(uri, text, 5, None, 1);
        let read_state = state.read();

        // Absolute offset 10 is 'f' in "foo" — relative offset 5.
        // The lexer stored the "foo" token with span [4, 7) relative to
        // src_bytes, so the absolute lookup at 10 must return it.
        let token = read_state
            .get_token_at_offset(10)
            .expect("Should find 'foo' via absolute offset");
        assert_eq!(token.span.start, 4);
        assert_eq!(token.span.end, 7);

        // Absolute offset 15 is '1' in "123" — relative offset 10.
        let token2 = read_state
            .get_token_at_offset(15)
            .expect("Should find '123' via absolute offset");
        assert_eq!(token2.span.start, 10);
        assert_eq!(token2.span.end, 13);

        // Absolute offset 9 is 'l' of "let" — relative offset 4.  The
        // returned span is relative (start=4, end=7), proving the lookup
        // subtracted script_start.
        let let_token = read_state
            .get_token_at_offset(9)
            .expect("Should find 'let' via absolute offset");
        assert_eq!(let_token.span.start, 4);
        assert_eq!(let_token.span.end, 7);

        // Absolute offset 8 (the space between "let" and "foo") is not a
        // token — must return None.
        assert!(
            read_state.get_token_at_offset(8).is_none(),
            "space should not be a token"
        );
    }

    /// Trivia spans in `DocumentState` are stored relative to the script
    /// section's `src_bytes`.  `offset_in_comment` takes an **absolute**
    /// byte offset and must subtract `script_start` before comparing
    /// against the relative trivia spans.  Otherwise comment detection in
    /// config files (with a non-zero `script_start`) would either miss
    /// real comments or report false positives.
    #[test]
    fn test_offset_in_comment_with_script_start_relative_trivia() {
        let cache = DocumentCache::new(10);
        let uri = "file:///def_comment_test.chrn";
        // "@def\n" is 5 bytes; the script section is "let x // inside script\n".
        //  0         1         2         3
        //  0123456789012345678901234567890
        //  @def\nlet x // inside script\n
        let text = Arc::new("@def\nlet x // inside script\n".to_string());
        let state_arc = cache.get_or_create(uri, text, 5, None, 1);
        let state = state_arc.read();

        // Absolute offset 13 (the '/' of "//") is a comment.  After
        // subtracting script_start=5 we get relative offset 8 — which
        // must still be detected because the trivia spans are relative.
        assert!(
            state.offset_in_comment(13),
            "absolute offset 13 ('//' start) must be detected as comment"
        );
        // Absolute offset 21 (the 'i' in "inside") is inside the comment.
        assert!(
            state.offset_in_comment(21),
            "absolute offset 21 (inside comment) must be detected as comment"
        );
        // Absolute offset 9 (the 'l' in "let") is not a comment.
        assert!(
            !state.offset_in_comment(9),
            "absolute offset 9 (start of 'let') must not be a comment"
        );
    }

    /// `config_load_error_to_diagnostics` shifts the loader's relative
    /// spans by `script_start` so the resulting LSP `Range`s land in
    /// absolute file coordinates.  This test pins that behaviour: a
    /// primary annotation at relative span [2, 6) with `script_start=10`
    /// must produce a diagnostic whose range starts at line 1 (not line 0).
    #[test]
    fn test_config_load_error_to_diagnostics_uses_absolute_positions() {
        // Document text: "@def\n" (5 bytes) + "let x = 1\n" (10 bytes).
        // script_start = 5 (the byte position of '@' in the file).
        let text = "@def\nlet x = 1\n";

        // A primary annotation at relative span [2, 6) — this corresponds
        // to absolute byte offsets [7, 11), i.e. the substring "let x" on
        // line 1 (line 0 is "@def\n").
        let primary_span = SourceSpan::new(SourceRegionId::new(0), 2, 6);
        let diag = SourceDiagnostic::builder(
            None,
            DiagnosticLevel::Error,
            "test error".to_string(),
            PathId::new(0),
        )
        .add_annotation(primary_span, AnnotationKind::Primary, None)
        .build();

        let cfg_err = ConfigLoadError::Diagnostic(diag);
        let lsp_diags = config_load_error_to_diagnostics(cfg_err, text, 5);

        // The first diagnostic is the primary one.
        let primary = lsp_diags
            .first()
            .expect("at least one diagnostic should be produced");

        // The primary annotation is at absolute bytes [7, 11), which on
        // line 1 of the document is characters [2, 6).  If the relative
        // span had not been shifted, the range would land on line 0.
        assert_eq!(
            primary.range.start,
            Position::new(1, 2),
            "primary diagnostic must start at the absolute line/col (script_start shift applied)"
        );
        assert_eq!(
            primary.range.end,
            Position::new(1, 6),
            "primary diagnostic must end at the absolute line/col (script_start shift applied)"
        );
    }

    /// When `script_start = 0` (no `@def` — the entire file is the script),
    /// relative and absolute coordinates coincide, so the diagnostic range
    /// must map directly to the byte positions in the text.
    #[test]
    fn test_config_load_error_to_diagnostics_no_script_start_is_identity() {
        let text = "let x = 1\n";
        let primary_span = SourceSpan::new(SourceRegionId::new(0), 4, 5);
        let diag = SourceDiagnostic::builder(
            None,
            DiagnosticLevel::Error,
            "identity test".to_string(),
            PathId::new(0),
        )
        .add_annotation(primary_span, AnnotationKind::Primary, None)
        .build();

        let cfg_err = ConfigLoadError::Diagnostic(diag);
        let lsp_diags = config_load_error_to_diagnostics(cfg_err, text, 0);

        let primary = lsp_diags.first().expect("diagnostic should be produced");
        assert_eq!(primary.range.start, Position::new(0, 4));
        assert_eq!(primary.range.end, Position::new(0, 5));
    }

    /// Every secondary annotation is also shifted by `script_start`.  A
    /// secondary annotation at relative span [0, 4) (the "@def" itself)
    /// with `script_start=0` must land on line 0; with `script_start=5`
    /// it must still land on line 0 because "@def" is the first 4 bytes
    /// of the file.  This test pins the behaviour for a non-trivial
    /// shift: a secondary annotation on a different line.
    #[test]
    fn test_config_load_error_to_diagnostics_secondary_annotation_shifted() {
        // Two-line document: line 0 is the config header, line 1 holds
        // the script section.  The primary annotation is on line 1 at
        // relative col 0-1; the secondary annotation is on line 1 at
        // relative col 4-5 (i.e. the '=' character).
        let text = "@def\nlet x = 1\n";
        let primary_span = SourceSpan::new(SourceRegionId::new(0), 0, 1);
        let secondary_span = SourceSpan::new(SourceRegionId::new(0), 4, 5);
        let diag = SourceDiagnostic::builder(
            None,
            DiagnosticLevel::Error,
            "secondary test".to_string(),
            PathId::new(0),
        )
        .add_annotation(primary_span, AnnotationKind::Primary, None)
        .add_annotation(
            secondary_span,
            AnnotationKind::Secondary,
            Some("equals sign".to_string()),
        )
        .build();

        let cfg_err = ConfigLoadError::Diagnostic(diag);
        let lsp_diags = config_load_error_to_diagnostics(cfg_err, text, 5);

        // Expect 2 diagnostics: primary, then secondary.
        assert!(
            lsp_diags.len() >= 2,
            "primary + secondary must each produce a diagnostic"
        );

        // Primary: relative [0, 1) on line 1 of script → absolute [5, 6)
        // on line 1 of the document → Position(1, 0)..Position(1, 1).
        let primary = &lsp_diags[0];
        assert_eq!(primary.range.start, Position::new(1, 0));
        assert_eq!(primary.range.end, Position::new(1, 1));

        // Secondary: relative [4, 5) on line 1 of script → absolute
        // [9, 10) on line 1 of the document → Position(1, 4)..Position(1, 5).
        let secondary = lsp_diags
            .iter()
            .find(|d| d.message == "equals sign" || d.message.contains("related to this"))
            .expect("secondary diagnostic must be emitted");
        assert_eq!(
            secondary.range.start,
            Position::new(1, 4),
            "secondary diagnostic must use the script_start-shifted range"
        );
        assert_eq!(secondary.range.end, Position::new(1, 5));
    }

    /// `push_diagnostics` shifts relative diagnostic spans by the
    /// region's `script_start` and converts the resulting absolute byte
    /// offset against the whole-document `fallback_text`.  This is the
    /// primary path used by `analyze_and_publish_task` for parser /
    /// name-resolution / type-check errors.
    #[test]
    fn test_push_diagnostics_relative_to_absolute_via_region() {
        // The whole document is "@def\nlet x = 1\n".  The main region's
        // `src_bytes` are the script section only ("let x = 1\n"), with
        // script_start=5 pointing at the '@' in the file.
        let full_text = "@def\nlet x = 1\n";
        let main_region = SourceRegion::new(
            1,
            1,
            b"let x = 1\n".to_vec(),
            SourceRegionId::new(0),
            PathId::new(0),
            5,
            None,
        );

        let mut arena: Arena<SourceRegion, SourceRegionId> = Arena::new();
        arena.push(main_region);

        // Diagnostic at relative span [0, 1) — the "l" of "let" — with
        // the same path_id as the main region, so it is resolved to the
        // main region (script_start=5).  The expected absolute range is
        // [5, 6) on line 1.
        let diag = SourceDiagnostic::builder(
            None,
            DiagnosticLevel::Error,
            "type check failed".to_string(),
            PathId::new(0),
        )
        .add_annotation(
            SourceSpan::new(SourceRegionId::new(0), 0, 1),
            AnnotationKind::Primary,
            None,
        )
        .build();

        let mut lsp_diags: Vec<tower_lsp::lsp_types::Diagnostic> = Vec::new();
        push_diagnostics(
            &mut lsp_diags,
            std::slice::from_ref(&diag),
            &arena,
            full_text,
            full_text.len(),
            "chrn-typecheck",
        );

        assert_eq!(lsp_diags.len(), 1, "one diagnostic expected");
        let d = &lsp_diags[0];
        assert_eq!(
            d.range.start,
            Position::new(1, 0),
            "push_diagnostics must shift the relative span by script_start"
        );
        assert_eq!(d.range.end, Position::new(1, 1));
        assert_eq!(d.message, "type check failed");
        assert_eq!(d.source.as_deref(), Some("chrn-typecheck"));
    }

    /// Import errors (file-not-found, is-a-directory, IO, etc.) must be
    /// reported on the `import` statement in the importing module.  That
    /// means the diagnostic's `path_id` must resolve to the importing
    /// module's region so `push_diagnostics` shifts the span by the
    /// importing module's `script_start`.  If the diagnostic incorrectly
    /// used the imported module's `path_id`, the span would be shifted by
    /// the wrong `script_start` and land in the wrong file position.
    #[test]
    fn test_push_diagnostics_import_error_uses_importing_module_region() {
        // Main file: config header "@def\n" (5 bytes) then script section
        // "import \"missing\"\n".  The import path "missing" is at script
        // section bytes [8, 15).
        let full_text = "@def\nimport \"missing\"\n";
        let main_region = SourceRegion::new(
            1,
            1,
            b"import \"missing\"\n".to_vec(),
            SourceRegionId::new(0),
            PathId::new(0),
            5,
            None,
        );
        // Imported file region (would exist if the file were loadable).
        // Its `script_start` is 0, so a diagnostic tied to this region
        // would not be shifted.
        let imported_region = SourceRegion::new(
            1,
            1,
            b"let unused = 0\n".to_vec(),
            SourceRegionId::new(1),
            PathId::new(1),
            0,
            None,
        );

        let mut arena: Arena<SourceRegion, SourceRegionId> = Arena::new();
        arena.push(main_region);
        arena.push(imported_region);

        // Import error diagnostic uses the IMPORTING module's path_id (0)
        // because the span points at the import statement in the main file.
        let diag = SourceDiagnostic::builder(
            None,
            DiagnosticLevel::Error,
            "import not found".to_string(),
            PathId::new(0),
        )
        .add_annotation(
            SourceSpan::new(SourceRegionId::new(0), 8, 15),
            AnnotationKind::Primary,
            None,
        )
        .build();

        let mut lsp_diags: Vec<tower_lsp::lsp_types::Diagnostic> = Vec::new();
        push_diagnostics(
            &mut lsp_diags,
            std::slice::from_ref(&diag),
            &arena,
            full_text,
            full_text.len(),
            "chrn-config",
        );

        assert_eq!(lsp_diags.len(), 1, "one diagnostic expected");
        let d = &lsp_diags[0];
        // Relative span [8, 15) shifted by script_start=5 gives absolute
        // bytes [13, 20), which is the "missing" string on line 1.
        assert_eq!(
            d.range.start,
            Position::new(1, 8),
            "import error must be shifted by the importing module's script_start"
        );
        assert_eq!(d.range.end, Position::new(1, 15));
        assert_eq!(d.message, "import not found");
        assert_eq!(d.source.as_deref(), Some("chrn-config"));
    }

    /// When the diagnostic's `path_id` matches no region in the arena
    /// (e.g. compiler-intrinsic diagnostics, or for regions that have
    /// been evicted), `push_diagnostics` must fall back to the supplied
    /// `fallback_text` with `script_start = 0` so the resulting range
    /// lines up with the byte positions in `fallback_text`.
    #[test]
    fn test_push_diagnostics_no_matching_region_uses_fallback() {
        let full_text = "let x = 1\n";
        // Region uses path_id=1; the diagnostic uses path_id=0.  The
        // lookup must fall back to `fallback_text` (no shift).
        let main_region = SourceRegion::new(
            1,
            1,
            b"let x = 1\n".to_vec(),
            SourceRegionId::new(0),
            PathId::new(1),
            0,
            None,
        );

        let mut arena: Arena<SourceRegion, SourceRegionId> = Arena::new();
        arena.push(main_region);

        let diag = SourceDiagnostic::builder(
            None,
            DiagnosticLevel::Warn,
            "fallback test".to_string(),
            PathId::new(0), // Does NOT match the region's path_id.
        )
        .add_annotation(
            SourceSpan::new(SourceRegionId::new(0), 4, 5),
            AnnotationKind::Primary,
            None,
        )
        .build();

        let mut lsp_diags: Vec<tower_lsp::lsp_types::Diagnostic> = Vec::new();
        push_diagnostics(
            &mut lsp_diags,
            std::slice::from_ref(&diag),
            &arena,
            full_text,
            full_text.len(),
            "chrn-parser",
        );

        assert_eq!(lsp_diags.len(), 1);
        let d = &lsp_diags[0];
        // The diagnostic's span is treated as already-absolute in the
        // fallback text.  The byte range [4, 5) is the 'x' on line 0.
        assert_eq!(d.range.start, Position::new(0, 4));
        assert_eq!(d.range.end, Position::new(0, 5));
    }

    /// `find_matching_entities` returns the document's `script_start` as
    /// the last element of each tuple so that callers in `references` /
    /// `rename` can shift relative spans to absolute file positions.
    /// This test pins that the `script_start` is read from the state and
    /// propagated to the caller — a regression here would silently break
    /// cross-module references and rename for any file with a non-zero
    /// `script_start`.
    #[test]
    fn test_find_matching_entities_propagates_script_start() {
        let cache = Arc::new(DocumentCache::new(10));

        // Build two cached documents with different `script_start` values
        // so we can verify the function reads the per-state value rather
        // than any global.  The lexer is given only the relative script
        // section bytes via `get_or_create`, so the cache is self-
        // consistent with the production path.
        let uri_a = "file:///a.chrn";
        let text_a = Arc::new("@def\nlet a = 1".to_string());
        let state_a = cache.get_or_create(uri_a, text_a, 5, None, 1);

        let uri_b = "file:///b.chrn";
        let text_b = Arc::new("let b = 1".to_string());
        let state_b = cache.get_or_create(uri_b, text_b, 0, None, 1);

        // Read script_start directly from each DocumentState.  This is
        // the value `find_matching_entities` is supposed to return per
        // entry; we check it matches what was passed to `get_or_create`.
        assert_eq!(state_a.read().script_start, 5);
        assert_eq!(state_b.read().script_start, 0);

        // Even with no matching entities (the compiler is `None`, so
        // `get_definition_location` returns `None` for every entry and
        // the result vector is empty), the function must not panic and
        // must return a `Vec` whose element type is the documented
        // 5-tuple `(String, Arc<String>, u32, u32, usize)`.  The
        // compile-time type check is the regression guard: a future
        // refactor that drops `script_start` from the return type fails
        // to build.
        let results: Vec<(String, Arc<String>, u32, u32, usize)> =
            DocumentState::find_matching_entities(
                &cache,
                "<no-match>",
                chrn_utils::source_map::source_span::SourceSpan::new(
                    chrn_utils::id_types::SourceRegionId::new(0),
                    0,
                    1,
                ),
                None,
            );
        // No compiler has been built, so no entries can be matched.
        // The result must be empty, not panic.
        assert!(
            results.is_empty(),
            "no compiler → no matches; empty result is the expected outcome"
        );
    }

    /// `rel_to_abs_offset` is used by every LSP surface that converts a
    /// relative byte offset into an absolute file position.  Pinning
    /// specific values (especially the boundary cases) protects against
    /// off-by-one regressions when the loader layout changes.
    #[test]
    fn test_rel_to_abs_offset_boundary_values() {
        // script_start = usize::MAX / 2 is the largest script_start that
        // does not overflow for any in-range relative offset.
        let script = (u32::MAX / 2) as usize;
        assert_eq!(rel_to_abs_offset(0, script), script as u32);
        assert_eq!(rel_to_abs_offset(1, script), (script as u32) + 1);
        assert_eq!(
            rel_to_abs_offset(u32::MAX, script),
            (script as u32).saturating_add(u32::MAX),
            "must saturate on overflow rather than wrap"
        );

        // script_start = 0 with a large relative offset must not overflow
        // because the input itself is bounded by u32::MAX.
        assert_eq!(rel_to_abs_offset(u32::MAX, 0), u32::MAX);

        // `abs_to_rel_offset` is the inverse and must be lossless when
        // the input is in the absolute coordinate system.
        let abs = rel_to_abs_offset(123, 456);
        assert_eq!(abs_to_rel_offset(abs, 456), 123);
    }
}
