use unicode_width::UnicodeWidthChar;

use crate::{color, symbols::Span};

const TOTAL_SEPARATORS: usize = 60;

pub struct LineData {
    fmt_segment: String,
    ln: usize,
    col: usize,
}

struct LineSpan<'a> {
    ln: usize,
    span: &'a Span,
}

//NOTE: MOST OF THIS WAS INDUCTIVE, MAKE SURE THIS DOES NOT BREAK.

// Ability to choose color when help exists in a better form
/// Returns line, column and red arrows under given spans, with the rest of the line also shown.
pub fn form_err_diag(src_bytes: &[u8], spans: &[Span], can_color: bool) -> LineData {
    let src_str = str::from_utf8(src_bytes).unwrap_or("<invalid source file>");

    let mut line_spans: Vec<LineSpan> = Vec::new();
    for span in spans {
        let (ln_start, ln_end) = get_src_line_info(src_bytes, span);
        line_spans.push(LineSpan { ln: ln_start, span });

        if ln_start != ln_end {
            line_spans.push(LineSpan { ln: ln_end, span });
        }
    }

    // Group by line number with relative positions
    let mut ln_groups: Vec<(usize, Vec<(usize, usize)>)> = Vec::new();
    for ls in &line_spans {
        let ln_start_byte = get_start_of_line(src_bytes, ls.span.start);
        let ln_last_byte = get_line_end(src_bytes, ln_start_byte);

        let rel_start = ls.span.start - ln_start_byte;
        let rel_end = if ls.span.end < ln_last_byte {
            ls.span.end - ln_start_byte
        } else {
            ln_last_byte - ln_start_byte
        };

        if let Some(group) = ln_groups.iter_mut().find(|(n, _)| *n == ls.ln) {
            group.1.push((rel_start, rel_end));
        } else {
            ln_groups.push((ls.ln, vec![(rel_start, rel_end)]));
        }
    }

    ln_groups.sort_by_key(|(ln, _)| *ln);

    let first_ln_num = ln_groups.first().expect("Cannot have < 1 spans").0;
    let last_ln_num = ln_groups.last().expect("Cannot have < 1 spans").0;
    // Ensures width is at least 3 or more
    let ln_width = last_ln_num.to_string().len().max(3);

    // Format each line group
    let mut fmt_segments: Vec<(usize, String)> = Vec::new();
    for (ln_num, ranges) in &ln_groups {
        let span = line_spans
            .iter()
            .find(|ls| ls.ln == *ln_num)
            .expect("Line number already exists")
            .span;

        let ln_start_byte = get_start_of_line(src_bytes, span.start);
        let ln_last_byte = get_line_end(src_bytes, ln_start_byte);
        let ln_str = str::from_utf8(&src_bytes[ln_start_byte..ln_last_byte]).expect("Lexer broke");

        let merged = merge_ranges(ranges);
        fmt_segments.push((
            *ln_num,
            format_line(*ln_num, ln_str, &merged, ln_width, can_color),
        ));
    }

    // Join segments with dashes between non-consecutive lines
    let mut fmt_segment = String::new();
    for (i, (ln_num, segment)) in fmt_segments.iter().enumerate() {
        if i > 0 {
            let prev_ln = fmt_segments[i - 1].0;
            let separator = if *ln_num > prev_ln + 1 {
                "\n~~~~\n"
            } else {
                "\n"
            };

            fmt_segment.push_str(separator);
        }
        fmt_segment.push_str(segment);
    }

    let first_ln_span_start = spans.iter().map(|s| s.start).min().expect("Exists");
    let first_ln_start_byte = get_start_of_line(src_bytes, first_ln_span_start);
    let col = char_width_offset(src_str, first_ln_start_byte, first_ln_span_start) + 1;

    LineData {
        ln: first_ln_num,
        col,
        fmt_segment,
    }
}

pub fn standardize_err(base_msg: &str, line_data: &LineData, help: &str) -> String {
    format!(
        "{base_msg}\n[{}:{}]\n{}\n{help}{}",
        line_data.ln,
        line_data.col,
        line_data.fmt_segment,
        "-".repeat(TOTAL_SEPARATORS)
    )
}

pub fn standardize_help(msg: &str, can_color: bool) -> String {
    let (orange, nc) = color::get_orange(can_color);

    if can_color {
        format!("{orange}help{nc}: {msg}\n")
    } else {
        format!("help: {msg}\n")
    }
}

fn get_src_line_info(src: &[u8], span: &Span) -> (usize, usize) {
    let mut ln_end = 1;
    let mut i = 0;

    while i <= span.end {
        match src[i] {
            b'\n' => {
                i += 1;
                ln_end += 1;
            }
            b'\r' if src.get(i + 1) == Some(&b'\n') => {
                i += 2;
                ln_end += 1;
            }
            _ => i += 1,
        }
    }

    let mut ln_start = ln_end;
    for i in (span.start..span.end).rev() {
        if src[i] == b'\n' {
            ln_start -= 1;
        }
    }

    (ln_start, ln_end)
}

fn get_start_of_line(src: &[u8], span_start: usize) -> usize {
    for i in (1..=span_start).rev() {
        if src[i - 1] == b'\n' {
            return i;
        }
    }
    0
}

fn get_line_end(src: &[u8], start: usize) -> usize {
    for i in start..src.len() {
        match src[i] {
            b'\r' if src.get(i + 1) == Some(&b'\n') => return i,
            b'\n' => return i,
            _ => {}
        }
    }
    src.len()
}

fn char_width_offset(s: &str, start: usize, end: usize) -> usize {
    s[start..end]
        .chars()
        .map(|c| UnicodeWidthChar::width(c).unwrap_or(1))
        .sum()
}

fn merge_ranges(ranges: &[(usize, usize)]) -> Vec<(usize, usize)> {
    let mut sorted = ranges.to_vec();
    sorted.sort_by_key(|r| r.0);
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for range in sorted {
        if let Some(last) = merged.last_mut() {
            if range.0 <= last.1 + 1 {
                last.1 = last.1.max(range.1);
                continue;
            }
        }
        merged.push(range);
    }
    merged
}

fn format_line(
    ln_num: usize,
    ln_str: &str,
    ranges: &[(usize, usize)],
    ln_width: usize,
    can_color: bool,
) -> String {
    let bar_spacing = " ".repeat(ln_width);
    let (red, nc) = color::get_red(can_color);

    let mut arrow_line = String::new();
    let mut last_end = 0;

    for &(start, end) in ranges {
        let adj_end = if end + 1 > ln_str.len() { end } else { end + 1 };

        arrow_line.push_str(&" ".repeat(char_width_offset(ln_str, last_end, start)));
        arrow_line.push_str(red);

        arrow_line.push_str(&"^".repeat(char_width_offset(ln_str, start, adj_end)));
        arrow_line.push_str(nc);

        last_end = adj_end;
    }

    format!(" {bar_spacing}|\n{ln_num:>ln_width$} |\t{ln_str}\n {bar_spacing}|\t{arrow_line}")
}
