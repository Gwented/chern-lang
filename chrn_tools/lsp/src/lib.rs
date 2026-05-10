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
    use crate::state::DocumentCache;
    use crate::text::{extract_word_at, offset_to_position, position_to_offset};
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
    fn test_document_cache_basic() {
        let cache = DocumentCache::new(2);
        let uri1 = "file:///test1.chrn";
        let text1 = Arc::new("let x = 1".to_string());

        let state1 = cache.get_or_create(uri1, text1.clone(), 0, None, 1);
        assert_eq!(state1.read().version, 1);

        // Retrieve existing
        let state1_again = cache.get(uri1).unwrap();
        assert!(Arc::ptr_eq(&state1, &state1_again));

        // Update version
        let state1_v2 = cache.get_or_create(uri1, text1.clone(), 0, None, 2);
        // It shouldn't update version if text is same?
        // Actually DocumentCache::get_or_create compares text.
        // If text is same, it returns existing state.
        assert_eq!(state1_v2.read().version, 1);

        // New document
        let uri2 = "file:///test2.chrn";
        let text2 = Arc::new("let y = 2".to_string());
        cache.get_or_create(uri2, text2, 0, None, 1);

        // New document, should evict uri1 if size > 2
        let uri3 = "file:///test3.chrn";
        let text3 = Arc::new("let z = 3".to_string());
        cache.get_or_create(uri3, text3, 0, None, 1);

        // uri1 should be evicted (LRU or just first-in depending on implementation)
        // The implementation uses:
        // let keys_to_remove: Vec<String> = cache.docs.keys().take(to_remove).map(|k| k.to_string()).collect();
        // which is arbitrary for HashMap, but usually evicts something.
        assert!(
            cache.get(uri1).is_none() || cache.get(uri2).is_none() || cache.get(uri3).is_none()
        );
        // Since we have 3 docs and max_size 2, one must be gone.
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

    // IS THAT A CRAB
    // ITS A CRAB
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
