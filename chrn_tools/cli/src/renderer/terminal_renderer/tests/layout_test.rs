use chrn_utils::source_map::source_diagnostic::annotations::Annotation;
use unicode_width::UnicodeWidthStr;

use crate::renderer::terminal_renderer::{
    layout::{self, RenderInfo, RenderLineLayout},
    tests::helpers::{full_span, line_at, ln_view, primary, secondary, span},
};

/// Walks each assigned layer the way the row builder does and asserts the invariant it now
/// depends on outright: placements arrive left to right and never reach back behind the cursor.
///
/// This is what replaced the `saturating_sub` / `max(cursor)` guards in `render_line_layout_text`.
fn assert_layers_are_walkable(ln_layout: &RenderLineLayout, src_str: &str) {
    let ln_end = layout::visual_ln_end(ln_layout.ln, src_str);
    // cursors[i] is the column layer i has been written up to.
    let mut cursors: Vec<usize> = Vec::new();

    for info in &ln_layout.render_info {
        let placement = layout::place_annotation(info.annotation, ln_layout.ln, ln_end, src_str);
        let idx = info.layer as usize;
        if idx >= cursors.len() {
            cursors.resize(idx + 1, 0);
        }

        assert!(
            placement.start >= cursors[idx],
            "layer {idx}: {placement:?} starts behind cursor {}",
            cursors[idx]
        );

        let label_cols = match &info.annotation.label {
            Some(label) => 1 + UnicodeWidthStr::width(label.as_str()),
            None => 0,
        };
        cursors[idx] = placement.start + placement.ptr_len + label_cols;
    }
}

/// Runs layer assignment over `annotations` on the line holding `probe_byte`, then checks the
/// walkability invariant and hands back the layer each annotation landed on, in render order.
fn assign_and_collect(src: &str, probe_byte: u32, annotations: &[Annotation]) -> Vec<u32> {
    let view = ln_view(src, full_span(src));
    let ln = line_at(&view, probe_byte);

    let render_info = annotations
        .iter()
        .map(|ann| RenderInfo::new(0, ann))
        .collect();
    let mut ln_layout = RenderLineLayout::new(ln, render_info);

    layout::assign_layers_in_layout(&mut ln_layout, src);
    assert_layers_are_walkable(&ln_layout, src);

    ln_layout.render_info.iter().map(|i| i.layer).collect()
}

#[test]
fn visual_ln_end_drops_a_trailing_newline() {
    let src = "abc\ndef";
    let view = ln_view(src, full_span(src));

    let first = line_at(&view, 0);
    // The mapper hands back [0, 4) for "abc\n" only when the line ends the region; either way the
    // visual end must land on the byte after 'c'.
    assert_eq!(layout::visual_ln_end(first, src), 3);

    let last = line_at(&view, 4);
    assert_eq!(layout::visual_ln_end(last, src), src.len());
}

#[test]
fn single_line_span_claims_its_own_columns() {
    let src = "bad: notatype\n";
    let view = ln_view(src, full_span(src));
    let ln = line_at(&view, 0);
    let ln_end = layout::visual_ln_end(ln, src);

    // "notatype" starts at byte 5 and runs 8 bytes.
    let ann = primary(5, 13, None);
    let placement = layout::place_annotation(&ann, ln, ln_end, src);

    assert_eq!(placement.start, 5);
    assert_eq!(placement.ptr_len, 8);
    assert_eq!(placement.end, 13);
}

#[test]
fn multi_line_span_is_intersected_with_each_line() {
    let src = "let x = 1 +\n  2 + \"a\"\n";
    let view = ln_view(src, span(8, 15));

    // Byte 8 through 15 covers "1 +\n  2", so it reaches the end of line 1 and the start of line 2.
    let ann = primary(8, 15, None);

    let first = line_at(&view, 8);
    let first_end = layout::visual_ln_end(first, src);
    let on_first = layout::place_annotation(&ann, first, first_end, src);
    // Starts at "1", stops where the line's text does rather than running past the newline.
    assert_eq!(on_first.start, 8);
    assert_eq!(on_first.ptr_len, 3);

    let second = line_at(&view, 14);
    let second_end = layout::visual_ln_end(second, src);
    let on_second = layout::place_annotation(&ann, second, second_end, src);
    // Starts at column 0 of line 2 rather than at the span's own start, which is on line 1.
    assert_eq!(on_second.start, 0);
    assert_eq!(on_second.ptr_len, 3);
}

#[test]
fn interior_lines_of_a_multi_line_span_carry_no_pointer() {
    let src = "aaa\nbbb\nccc\n";
    let view = ln_view(src, span(1, 10));
    let ann = primary(1, 10, None);

    // Line 2 holds neither endpoint, so layer assignment drops it and the layout is discarded.
    let middle = line_at(&view, 4);
    let mut ln_layout = RenderLineLayout::new(middle, vec![RenderInfo::new(0, &ann)]);
    layout::assign_layers_in_layout(&mut ln_layout, src);

    assert!(ln_layout.render_info.is_empty());
}

#[test]
fn unicode_width_drives_columns_not_byte_length() {
    let src = "日本 = 1\n";
    let view = ln_view(src, full_span(src));
    let ln = line_at(&view, 0);
    let ln_end = layout::visual_ln_end(ln, src);

    // Two wide chars, six bytes, four columns.
    let wide = primary(0, 6, None);
    let on_wide = layout::place_annotation(&wide, ln, ln_end, src);
    assert_eq!(on_wide.start, 0);
    assert_eq!(on_wide.ptr_len, 4);

    // "1" sits at byte 9 but column 7, because the two wide chars count double.
    let digit = primary(9, 10, None);
    let on_digit = layout::place_annotation(&digit, ln, ln_end, src);
    assert_eq!(on_digit.start, 7);
    assert_eq!(on_digit.ptr_len, 1);
}

#[test]
fn span_over_the_stripped_newline_keeps_one_column() {
    let src = "var->\nname: str [\n";
    let view = ln_view(src, full_span(src));
    let ln = line_at(&view, 16);
    let ln_end = layout::visual_ln_end(ln, src);

    // The `<eof>` pointer every parser error uses: a one-byte span over the trailing newline,
    // which `visual_ln_end` strips. It must still be drawn, one column past the line's text.
    let ann = primary(17, 18, None);
    let placement = layout::place_annotation(&ann, ln, ln_end, src);

    assert_eq!(placement.start, 11);
    assert_eq!(placement.ptr_len, 1);
    assert_eq!(placement.end, 12);
}

#[test]
fn label_reserves_columns_past_the_pointer() {
    let src = "abcd\n";
    let view = ln_view(src, full_span(src));
    let ln = line_at(&view, 0);
    let ln_end = layout::visual_ln_end(ln, src);

    let bare = layout::place_annotation(&primary(0, 4, None), ln, ln_end, src);
    let labelled = layout::place_annotation(&primary(0, 4, Some("hi")), ln, ln_end, src);

    // One space before the label, the label itself, and one column of separation after it.
    assert_eq!(labelled.end - bare.end, 1 + 2 + 1);
}

#[test]
fn annotations_that_do_not_collide_share_a_row() {
    let src = "aaaa bbbb cccc\n";
    let layers = assign_and_collect(src, 0, &[primary(0, 4, None), secondary(10, 14, None)]);

    assert_eq!(layers, vec![0, 0]);
}

#[test]
fn labels_that_would_collide_get_their_own_rows() {
    let src = "aaaa bbbb cccc\n";
    // Without labels these two fit on one row; "first" runs past column 10 and forces a split.
    let layers = assign_and_collect(
        src,
        0,
        &[
            primary(0, 4, Some("first")),
            secondary(10, 14, Some("third")),
        ],
    );

    assert_eq!(layers, vec![0, 1]);
}

#[test]
fn primary_takes_the_topmost_row() {
    let src = "aaaa bbbb cccc\n";
    // Secondary is listed first and starts further left, but the primary still claims layer 0.
    let annotations = [
        secondary(0, 4, Some("second")),
        primary(10, 14, Some("first")),
    ];

    let view = ln_view(src, full_span(src));
    let ln = line_at(&view, 0);
    let render_info = annotations
        .iter()
        .map(|ann| RenderInfo::new(0, ann))
        .collect();
    let mut ln_layout = RenderLineLayout::new(ln, render_info);

    layout::assign_layers_in_layout(&mut ln_layout, src);
    assert_layers_are_walkable(&ln_layout, src);

    let top = ln_layout
        .render_info
        .iter()
        .find(|info| info.layer == 0)
        .expect("some annotation must hold layer 0");
    assert_eq!(top.annotation.span, span(10, 14));
}

#[test]
fn stacked_annotations_stay_walkable() {
    let src = "aaaa bbbb cccc dddd\n";
    // Four overlapping labels, which is the densest arrangement the layer assigner has to handle.
    let layers = assign_and_collect(
        src,
        0,
        &[
            primary(0, 4, Some("one")),
            secondary(5, 9, Some("two")),
            secondary(10, 14, Some("three")),
            secondary(15, 19, Some("four")),
        ],
    );

    assert_eq!(layers.len(), 4);
    // Every annotation must land on some row, and no row may be skipped.
    let mut sorted = layers.clone();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(sorted, (0..sorted.len() as u32).collect::<Vec<u32>>());
}
