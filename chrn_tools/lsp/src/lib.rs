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
}
