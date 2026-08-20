use chrn_utils::source_map::source_diagnostic::footers::FooterKind;

use crate::renderer::terminal_renderer::{
    render_terminal_diags,
    tests::helpers::{
        TEST_PATH, body, diag, interner, plain_cfg, primary, render, render_at, secondary,
    },
};

#[test]
fn single_line_span_underlines_its_columns() {
    let src = "var->\nbad: notatype\n";
    let rendered = render(src, diag("no such type", vec![primary(11, 19, None)]));

    assert_eq!(
        body(&rendered),
        "  |
2 | bad: notatype
  |      ^^^^^^^^"
    );
}

#[test]
fn multi_line_span_is_underlined_on_every_line_it_touches() {
    let src = "let x = 1 +\n  2 + \"a\"\n";
    // The left operand spans the newline; the right operand sits on line 2 beside it.
    let rendered = render(
        src,
        diag(
            "mismatched operands",
            vec![primary(8, 15, None), primary(18, 21, None)],
        ),
    );

    assert_eq!(
        body(&rendered),
        "  |
1 | let x = 1 +
  |         ^^^
2 |   2 + \"a\"
  | ^^^   ^^^"
    );
}

#[test]
fn interior_lines_of_a_multi_line_span_are_elided() {
    let src = "aaa\nbbb\nccc\n";
    // Line 2 is fully inside the span but holds neither endpoint, so it drops out and the gap
    // separator takes its place.
    let rendered = render(src, diag("spans three lines", vec![primary(1, 10, None)]));

    assert_eq!(
        body(&rendered),
        "  |
1 | aaa
  |  ^^
 ---
3 | ccc
  | ^^"
    );
}

#[test]
fn eof_span_still_draws_a_pointer() {
    let src = "var->\nname: str [\n";
    // Byte 17 is the trailing newline, which the renderer strips from the printed line. The
    // pointer used to vanish entirely here; it now sits one column past the '['.
    let rendered = render(
        src,
        diag(
            "expected expression, found <eof>",
            vec![
                primary(17, 18, Some("Unexpected <eof>")),
                secondary(16, 17, Some("Token before <eof>")),
            ],
        ),
    );

    assert_eq!(
        body(&rendered),
        "  |
2 | name: str [
  |            ^ Unexpected <eof>
  |           - Token before <eof>"
    );
}

#[test]
fn wide_characters_shift_later_columns() {
    let src = "日本 = 1\n";
    let rendered = render(
        src,
        diag("wide", vec![primary(0, 6, None), primary(9, 10, None)]),
    );

    assert_eq!(
        body(&rendered),
        "  |
1 | 日本 = 1
  | ^^^^   ^"
    );
}

#[test]
fn non_overlapping_annotations_share_one_row() {
    let src = "aaaa bbbb cccc\n";
    let rendered = render(
        src,
        diag(
            "two spots",
            vec![primary(0, 4, None), secondary(10, 14, None)],
        ),
    );

    assert_eq!(
        body(&rendered),
        "  |
1 | aaaa bbbb cccc
  | ^^^^      ----"
    );
}

#[test]
fn labels_that_would_collide_stack_into_rows() {
    let src = "aaaa bbbb cccc\n";
    let rendered = render(
        src,
        diag(
            "two labelled spots",
            vec![
                primary(0, 4, Some("first")),
                secondary(10, 14, Some("third")),
            ],
        ),
    );

    assert_eq!(
        body(&rendered),
        "  |
1 | aaaa bbbb cccc
  | ^^^^ first
  |           ---- third"
    );
}

#[test]
fn non_adjacent_annotated_lines_get_a_gap_separator() {
    let src = "aa\nbb\ncc\n";
    let rendered = render(
        src,
        diag(
            "skips a line",
            vec![primary(0, 2, None), primary(6, 8, None)],
        ),
    );

    assert_eq!(
        body(&rendered),
        "  |
1 | aa
  | ^^
 ---
3 | cc
  | ^^"
    );
}

#[test]
fn region_start_offsets_line_numbers_and_widens_the_column() {
    let src = "var->\nbad: notatype\n";
    // What `@def` extraction produces: the region's first line is line 15 of the data file, so
    // the annotated line prints as 16 and the number column grows to two.
    let rendered = render_at(src, diag("no such type", vec![primary(11, 19, None)]), 15);

    assert_eq!(
        body(&rendered),
        "   |
16 | bad: notatype
   |      ^^^^^^^^"
    );
}

#[test]
fn diagnostics_without_a_region_arena_render_header_only() {
    let rendered = render_terminal_diags(
        &[diag("could not read file", Vec::new())],
        &[],
        None,
        &interner(),
        &plain_cfg(),
    );

    assert_eq!(
        rendered,
        vec![format!(
            "PATH => \"{TEST_PATH}\"\nerror: could not read file"
        )]
    );
}
