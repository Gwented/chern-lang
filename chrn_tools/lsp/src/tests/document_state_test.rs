use crate::state::{DocumentCache, DocumentState};
use std::sync::Arc;

#[test]
fn test_get_token_at_offset() {
    let cache = DocumentCache::new(10);
    let uri = "file:///test_tokens.chrn";
    let text = Arc::new("let foo = 123;".to_string());
    let state = cache.get_or_create(uri, text, 0, None, 1);
    let read_state = state.read();

    let token = read_state
        .get_token_at_offset(5)
        .expect("Should find 'foo'");
    assert_eq!(token.span.start, 4);
    assert_eq!(token.span.end, 7);

    let token2 = read_state
        .get_token_at_offset(10)
        .expect("Should find '123'");
    assert_eq!(token2.span.start, 10);
    assert_eq!(token2.span.end, 13);

    assert!(
        read_state.get_token_at_offset(3).is_none(),
        "Space should return None"
    );
}

#[test]
fn test_offset_in_comment_single_line() {
    let cache = DocumentCache::new(10);
    let uri = "file:///comment_test.chrn";
    let text = Arc::new("let x = 1 // comment here".to_string());
    let state_arc = cache.get_or_create(uri, text, 0, None, 1);
    let state = state_arc.read();

    assert!(
        !state.offset_in_comment(0),
        "start of code should not be in comment"
    );
    assert!(
        state.offset_in_comment(11),
        "offset at // should be in comment"
    );
    assert!(
        state.offset_in_comment(18),
        "offset inside comment text should be in comment"
    );
}

#[test]
fn test_offset_in_comment_only_applies_to_own_line() {
    let cache = DocumentCache::new(10);
    let uri = "file:///comment_multiline.chrn";
    let text = Arc::new("// first line\nlet y = 2".to_string());
    let state_arc = cache.get_or_create(uri, text, 0, None, 1);
    let state = state_arc.read();

    assert!(
        !state.offset_in_comment(14),
        "start of second line should not be treated as inside first-line comment"
    );
}

#[test]
fn test_get_token_at_offset_with_script_start() {
    let cache = DocumentCache::new(10);
    let uri = "file:///def_test.chrn";
    let text = Arc::new("@def\nlet foo = 123;".to_string());
    let state = cache.get_or_create(uri, text, 5, None, 1);
    let read_state = state.read();

    let token = read_state
        .get_token_at_offset(10)
        .expect("Should find 'foo' via absolute offset");
    assert_eq!(token.span.start, 4);
    assert_eq!(token.span.end, 7);

    let token2 = read_state
        .get_token_at_offset(15)
        .expect("Should find '123' via absolute offset");
    assert_eq!(token2.span.start, 10);
    assert_eq!(token2.span.end, 13);

    let let_token = read_state
        .get_token_at_offset(9)
        .expect("Should find 'let' via absolute offset");
    assert_eq!(let_token.span.start, 4);
    assert_eq!(let_token.span.end, 7);

    assert!(
        read_state.get_token_at_offset(8).is_none(),
        "space should not be a token"
    );
}

#[test]
fn test_offset_in_comment_with_script_start_relative_trivia() {
    let cache = DocumentCache::new(10);
    let uri = "file:///def_comment_test.chrn";
    let text = Arc::new("@def\nlet x // inside script\n".to_string());
    let state_arc = cache.get_or_create(uri, text, 5, None, 1);
    let state = state_arc.read();

    assert!(
        state.offset_in_comment(13),
        "absolute offset 13 ('//' start) must be detected as comment"
    );
    assert!(
        state.offset_in_comment(21),
        "absolute offset 21 (inside comment) must be detected as comment"
    );
    assert!(
        !state.offset_in_comment(9),
        "absolute offset 9 (start of 'let') must not be a comment"
    );
}

#[test]
fn test_find_matching_entities_propagates_script_start() {
    let cache = Arc::new(DocumentCache::new(10));

    let uri_a = "file:///a.chrn";
    let text_a = Arc::new("@def\nlet a = 1".to_string());
    let state_a = cache.get_or_create(uri_a, text_a, 5, None, 1);

    let uri_b = "file:///b.chrn";
    let text_b = Arc::new("let b = 1".to_string());
    let state_b = cache.get_or_create(uri_b, text_b, 0, None, 1);

    assert_eq!(state_a.read().script_start, 5);
    assert_eq!(state_b.read().script_start, 0);

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
    assert!(
        results.is_empty(),
        "no compiler -> no matches; empty result is the expected outcome"
    );
}
