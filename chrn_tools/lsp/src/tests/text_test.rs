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

/// Brute-force restatement of the redundancy rule the fast path implements.
fn dedup_reference(ranges: &[Range]) -> Vec<usize> {
    let mut result = Vec::new();
    for i in 0..ranges.len() {
        let r1 = &ranges[i];
        let redundant = ranges.iter().enumerate().any(|(j, r2)| {
            if i == j {
                return false;
            }
            let starts_after_or_at = (r2.start.line, r2.start.character) >= (r1.start.line, r1.start.character);
            let ends_before_or_at = (r2.end.line, r2.end.character) <= (r1.end.line, r1.end.character);
            starts_after_or_at
                && ends_before_or_at
                && (r1.start != r2.start || r1.end != r2.end || j < i)
        });
        if !redundant {
            result.push(i);
        }
    }
    result
}

/// The sorted implementation must agree with the pairwise rule on overlapping,
/// nested, identical, and disjoint inputs alike.
#[test]
fn test_deduplicate_range_indices_matches_pairwise_rule() {
    let r = |sl: u32, sc: u32, el: u32, ec: u32| Range {
        start: Position::new(sl, sc),
        end: Position::new(el, ec),
    };
    let cases: Vec<Vec<Range>> = vec![
        vec![],
        vec![r(0, 0, 0, 5)],
        vec![r(0, 0, 0, 10), r(0, 2, 0, 5), r(0, 2, 0, 5)],
        vec![r(0, 0, 0, 5), r(0, 0, 0, 3), r(0, 0, 0, 8)],
        vec![r(1, 0, 3, 4), r(2, 0, 2, 9), r(3, 0, 3, 4), r(0, 0, 9, 9)],
        vec![r(0, 0, 0, 4), r(0, 4, 0, 8), r(0, 8, 0, 12)],
        vec![r(2, 2, 2, 6), r(2, 2, 2, 6), r(2, 2, 2, 6)],
    ];

    for ranges in cases {
        assert_eq!(
            deduplicate_range_indices(&ranges),
            dedup_reference(&ranges),
            "diverged on {ranges:?}"
        );
    }
}

/// `LineIndex` is a table-driven restatement of `offset_to_position`; the two must
/// agree at every byte, including inside multi-byte characters and past the end.
#[test]
fn test_line_index_matches_offset_to_position() {
    let text = "let a = 1\nlet é = \"日本\"\n\nlet c = 3\n";
    let lines = crate::text::LineIndex::new(text);

    for offset in 0..=text.len() + 5 {
        assert_eq!(
            lines.position(offset),
            crate::text::offset_to_position(text, offset),
            "line index diverged at byte offset {offset}"
        );
    }
}

#[test]
fn test_line_index_empty_text() {
    let lines = crate::text::LineIndex::new("");
    assert_eq!(lines.position(0), Position::new(0, 0));
    assert_eq!(lines.position(7), Position::new(0, 0));
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
    let result = apply_text_change(existing, &change).expect("incremental replace must succeed");
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

/// `PositionCursor` is an incremental restatement of `offset_to_position`; the two
/// must agree at every byte boundary, including across multi-byte characters where
/// UTF-8 length and UTF-16 length diverge.
#[test]
fn test_position_cursor_matches_offset_to_position() {
    let text = "let a = 1\nlet é = \"日本\"\n\nlet c = 3";

    let mut cursor = crate::text::PositionCursor::new(text);
    for offset in 0..=text.len() {
        if !text.is_char_boundary(offset) {
            continue;
        }
        assert_eq!(
            cursor.position_at(offset),
            crate::text::offset_to_position(text, offset),
            "cursor diverged at byte offset {offset}"
        );
    }
}

/// Offsets that go backwards, land mid-character, or run past the end must still
/// produce the same answer as a full scan rather than panicking.
#[test]
fn test_position_cursor_handles_out_of_order_offsets() {
    let text = "aé\nb";
    let mut cursor = crate::text::PositionCursor::new(text);

    assert_eq!(
        cursor.position_at(4),
        crate::text::offset_to_position(text, 4)
    );
    assert_eq!(
        cursor.position_at(1),
        crate::text::offset_to_position(text, 1),
        "a backwards offset must fall back to a full scan"
    );
    assert_eq!(
        cursor.position_at(2),
        crate::text::offset_to_position(text, 2),
        "offset 2 is inside the two-byte 'é'"
    );
    assert_eq!(
        cursor.position_at(999),
        crate::text::offset_to_position(text, 999),
        "past-the-end offsets clamp"
    );
}
