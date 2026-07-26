use crate::text::{offset_to_position, position_to_offset};
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

#[test]
fn test_position_to_offset_line_past_end() {
    let text = "abc";
    assert_eq!(
        position_to_offset(text, Position::new(99, 0)),
        text.len(),
        "line past end must return text.len()"
    );
}

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
