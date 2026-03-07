use unicode_width::UnicodeWidthChar;

use crate::symbols::Span;

//FIX: ANSI
const RED: &str = "\x1b[31m";
const GREEN: &str = "\x1b[32m";
const ORANGE: &str = "\x1b[33m";
const NC: &str = "\x1b[0m";

const SEPARATORS: usize = 60;

//TODO: Handle multi-line errors
// Store \n array for binary search NOT now. DO NOT. do it now.

// FIXME: Given 'a: A "[Range()]' span.end reaches past the line causing a subtract overflow.
// Handling multi-line will likely fix it
pub fn form_err_diag(src: &[u8], span: &Span, can_color: bool) -> (usize, usize, String) {
    let src_str = str::from_utf8(src).expect("Lexer broke");

    let (seg_start, ln) = get_line_start(src, span);

    let seg_end = get_line_end(src, seg_start);

    let segment = &src[seg_start..seg_end];

    // WARN: Suspicious off by one...
    let col = src_str[seg_start..span.start].chars().count() + 1;

    let str_segment = str::from_utf8(segment).expect("Lexer broke");

    // Could both of these look less odd?
    let space_offset = src_str[seg_start..span.start]
        .chars()
        .map(|c| UnicodeWidthChar::width(c).unwrap_or(1))
        .sum();

    let spaces = " ".repeat(space_offset);

    // span range is inclusive exclusive so final character is missed without + 1
    // Has no other mathematical reasoning outside of this
    let arrow_offset = src_str[span.start..span.end + 1]
        .chars()
        .map(|c| UnicodeWidthChar::width(c).unwrap_or(1))
        .sum();

    let arrows = "^".repeat(arrow_offset);

    let fmt_segment = if can_color {
        format!("\t{str_segment}\n\t{spaces}{RED}{arrows}{NC}")
    } else {
        format!("\t{str_segment}\n\t{spaces}{arrows}")
    };

    println!("{}", &fmt_segment);

    (ln, col, fmt_segment)
}

//TEST:
pub fn form_help_diag(
    src: &[u8],
    span: &Span,
    msg: &str,
    add: bool,
    suggestion: &str,
    can_color: bool,
) -> String {
    let src_str = str::from_utf8(src).expect("Lexer broke");

    let (seg_start, _) = get_line_start(src, span);

    let op_count = suggestion.len();

    let ops = if add {
        "+".repeat(op_count)
    } else {
        "-".repeat(op_count)
    };

    let ops = if can_color {
        format!("{GREEN}{ops}{NC}")
    } else {
        format!("{RED}{ops}{NC}")
    };

    // Could both of these look less odd?
    let space_offset = src_str[seg_start..span.start]
        .chars()
        .map(|c| UnicodeWidthChar::width(c).unwrap_or(1))
        .sum();

    let spaces = " ".repeat(space_offset);

    let help = form_help(msg, can_color);

    let fmt_segment = format!("\t{spaces}{suggestion}\n\t{spaces}{ops}\n\t{help}");

    println!("{}", &fmt_segment);

    fmt_segment
}

pub fn form_help(msg: &str, can_color: bool) -> String {
    if can_color {
        format!("{ORANGE}Help{NC}: {msg}\n")
    } else {
        format!("Help: {msg}\n")
    }
}

/// Returns start of `span` in bytes from `src` and the line number it was on
fn get_line_start(src: &[u8], span: &Span) -> (usize, usize) {
    let mut ln = 1;

    let mut b: u8;

    let mut seg_start = 0;

    let mut i = 0;

    while i < span.end {
        b = src[i];

        if b == b'\n' {
            ln += 1;
            i += 1;
            seg_start = i;
        } else if b == b'\r' && src.get(i + 1).copied() == Some(b'\n') {
            ln += 1;
            i += 2;
            seg_start = i;
        } else {
            i += 1;
        }
    }

    (seg_start, ln)
}

fn get_line_end(original_text: &[u8], start: usize) -> usize {
    for i in start..original_text.len() {
        let b = original_text[i];

        if b == b'\r' && original_text.get(i + 1).copied() == Some(b'\n') {
            return i;
        } else if b == b'\n' {
            return i;
        }
    }

    //WARN: I don't remember why I returned this
    original_text.len()
}
