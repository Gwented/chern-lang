use crate::text::{
    apply_text_change, deduplicate_range_indices, extract_word_at, find_word_bounds,
};
use tower_lsp::lsp_types::{Position, Range, TextDocumentContentChangeEvent};

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
fn test_extract_word_at_clamp_past_end() {
    let line = "hello";
    let word = extract_word_at(line, 100);
    assert_eq!(word, "hello");
}

#[test]
fn test_extract_word_at_empty_line() {
    assert_eq!(extract_word_at("", 0), "");
}

#[test]
fn test_extract_word_at_directive_chars() {
    let line = "@def foo";
    assert_eq!(extract_word_at(line, 0), "@def");
    assert_eq!(extract_word_at(line, 5), "foo");
}

#[test]
fn test_find_word_bounds_basic() {
    let text = "let foo = 123";
    assert_eq!(find_word_bounds(text, 5), (4, 7));
    assert_eq!(find_word_bounds(text, 1), (0, 3));
    assert_eq!(find_word_bounds(text, 11), (10, 13));
}

#[test]
fn test_find_word_bounds_on_space() {
    let text = "a b";
    let (start, end) = find_word_bounds(text, 1);
    assert_eq!(start, 0, "start should walk back past preceding word chars");
    assert_eq!(end, 1, "end should not advance past a non-word char");
    assert_eq!(&text[start..end], "a");
}

#[test]
fn test_find_word_bounds_extended_chars() {
    let text = "@def foo";
    let (start, end) = find_word_bounds(text, 0);
    assert_eq!(&text[start..end], "@def");
}

#[test]
fn test_find_word_bounds_empty_text() {
    assert_eq!(find_word_bounds("", 0), (0, 0));
}

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
    let kept = deduplicate_range_indices(&ranges);
    assert_eq!(kept, vec![0, 1]);
}

#[test]
fn test_deduplicate_range_indices_removes_outer() {
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
    let ranges = vec![r, r];
    let kept = deduplicate_range_indices(&ranges);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0], 0);
}

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

#[test]
fn test_apply_text_change_incremental() {
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

#[test]
fn test_apply_text_change_deletion() {
    let existing = "let x = 1;\nlet y = 2;";
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
    _ = apply_text_change(existing, &change);
}
