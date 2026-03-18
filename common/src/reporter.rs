//TODO:
//STARTUP IDEA:
// Enums that guide the reporter on how to print info
use unicode_width::UnicodeWidthChar;

use crate::symbols::Span;

//FIX: ANSI
// Should these have a color.rs?
pub const RED: &str = "\x1b[31m";
pub const GREEN: &str = "\x1b[32m";
pub const ORANGE: &str = "\x1b[33m";
pub const NC: &str = "\x1b[0m";

const TOTAL_SEPARATORS: usize = 60;

//TODO: Store \n array for binary search NOT now. DO NOT. do it now.
// This is only within the metadata but nothing is done with it

pub struct LineData {
    fmt_segment: String,
    ln: usize,
    col: usize,
}

/// Returns line, column and red arrows under given span, with the rest of the line also shown.
pub fn form_err_diag(src_bytes: &[u8], span: &Span, can_color: bool) -> LineData {
    let src_str = str::from_utf8(src_bytes).unwrap_or("<invalid source file>");

    // first line number and last line number counting \n
    let (first_ln_num, last_ln_num) = get_src_line_info(src_bytes, span);

    let first_ln_start_byte = get_start_of_line(src_bytes, span.start);

    let line_amt = last_ln_num - first_ln_num + 1;

    let mut fmt_segments = Vec::new();

    let first_ln_last_byte = get_line_end(src_bytes, first_ln_start_byte);
    let first_ln_bytes = &src_bytes[first_ln_start_byte..first_ln_last_byte];

    //TODO: Should maybe just return the line here on error since unwrap_or seems wrong.
    let first_ln_str = str::from_utf8(first_ln_bytes).expect("Lexer broke");

    //NOTE: First line span start position relative to the line its on
    let span_start_rel = span.start - first_ln_start_byte;

    let span_end_rel = if span.end < first_ln_last_byte {
        span.end - first_ln_start_byte
    } else {
        first_ln_last_byte - first_ln_start_byte
    };

    //BUG: Starts here
    fmt_segments.push(format_line_segment(
        first_ln_num,
        first_ln_str,
        span_start_rel,
        span_end_rel,
        can_color,
    ));

    if line_amt > 2 {
        // Rice your error emitters.
        fmt_segments.push("---".to_string());
    }

    // NOTE: This only cares about two lines at most so that error messages don't span to infinity.
    // What if we had an `inf` keyword? What would it even do?
    if line_amt > 1 {
        let final_ln_start_byte = get_start_of_line(src_bytes, span.end);
        let final_ln_end_byte = get_line_end(src_bytes, final_ln_start_byte);

        let final_ln_bytes = if final_ln_start_byte < src_bytes.len() {
            &src_bytes[final_ln_start_byte..final_ln_end_byte]
        } else {
            &src_bytes[final_ln_start_byte..]
        };

        //TODO: Make this anything but an expect even though it is impossible to fail.
        let final_ln_str = str::from_utf8(final_ln_bytes).expect("Lexer broke");

        let err_start_byte = get_err_start(src_bytes, span.end);

        let final_span_start_rel = err_start_byte - final_ln_start_byte;
        let final_span_end_rel = span.end - final_ln_start_byte;

        fmt_segments.push(format_line_segment(
            last_ln_num,
            final_ln_str,
            final_span_start_rel,
            final_span_end_rel,
            can_color,
        ));
    }

    let fmt_segment = fmt_segments.join("\n");

    let col = char_width_offset(src_str, first_ln_start_byte, span.start) + 1;

    LineData {
        ln: first_ln_num,
        col,
        fmt_segment,
    }
}

pub fn standardize_err(base_msg: &str, line_data: &LineData, help: &str) -> String {
    let separators = "-".repeat(TOTAL_SEPARATORS);

    format!(
        "{base_msg}\n[{}:{}]\n{}\n{help}{separators}",
        line_data.ln, line_data.col, line_data.fmt_segment
    )
}

// TEST:
// Should look like
//
// (msg\n)
//
// -> {GREEN}+ [{NC}Range()]

pub fn form_suggest_diag(
    src_bytes: &[u8],
    span: &Span,
    op: &str,
    suggestion: &str,
    should_add: bool,
    can_color: bool,
) -> String {
    let end_byte = get_line_end(src_bytes, span.start);

    let rest_of_ln = str::from_utf8(&src_bytes[span.start..end_byte]).unwrap();

    let op_count = suggestion.len();

    let ops = op.repeat(op_count);

    let color = if should_add { GREEN } else { RED };

    // let suggest_header = if can_color {
    //     format!("---> {color}{suggestion}{NC}{rest_of_ln}")
    // } else {
    //     format!("-> {suggestion}{rest_of_ln}")
    // };
    //
    // let ops = if can_color {
    //     format!("{color}{ops}{NC}")
    // } else {
    //     format!("{ops}")
    // };
    //
    // let spaces = " ".repeat(suggestion.len());
    //
    // let help_diag = format!("{suggest_header}\n {spaces}{ops} <UNFINISHED>");
    //
    // help_diag
    todo!();
}

pub fn standardize_help(msg: &str, can_color: bool) -> String {
    if can_color {
        format!("\n{ORANGE}help{NC}: {msg}\n")
    } else {
        format!("\nhelp: {msg}\n")
    }
}

// get_full_src_span_info, get_full_src_info, get_src_info <-- THIS ONE. Actually nevermind
/// Returns the amount of lines between the start and end of the span.
fn get_src_line_info(src: &[u8], span: &Span) -> (usize, usize) {
    let mut ln_end = 1;

    let mut i = 0;

    //WARN: Suspicious LE usage
    while i <= span.end {
        let b = src[i];

        if b == b'\n' {
            i += 1;
            ln_end += 1;
        } else if b == b'\r' && src.get(i + 1).copied() == Some(b'\n') {
            i += 2;
            ln_end += 1;
        } else {
            i += 1;
        }
    }

    // line_start is set to line_end so if line_end is the start, nothing happens to line_start
    let mut ln_start = ln_end;

    for i in (span.start..span.end).rev() {
        if src[i] == b'\n' {
            ln_start -= 1;
        }
    }
    dbg!(ln_start, ln_end);
    // panic!();

    // This should be the line it starts on and the line it ends on.
    (ln_start, ln_end)
}

/// Returns the index of the start of the line of `span_start`
fn get_start_of_line(src: &[u8], span_start: usize) -> usize {
    // 30+ minutes debugging, and it was a single off by one error. Maybe it's time to stop.

    //NOTE: I don't know why this needs to be inclusive, because I didn't write down the first time
    //why I made 'get_full_src_info' inclusive, I just saw that it worked. Probably just related to
    //span inclusive, exclusive behavior causing other places to shift in math, which could be
    //inherently faulty, but this isn't priority.

    for i in (0..=span_start).rev() {
        if src[i - 1] == b'\n' {
            return i;
        }
    }
    // Defaults since this means there is only one byte present
    0
}

/// Collects bytes until a new line is reached and then returns the index of that byte.
fn get_line_end(src_bytes: &[u8], span_start: usize) -> usize {
    for i in span_start..src_bytes.len() {
        let b = src_bytes[i];

        if b == b'\r' && src_bytes.get(i + 1).copied() == Some(b'\n') {
            return i;
        } else if b == b'\n' {
            return i;
        }
    }

    // Returning this because if the length is made it to then...
    src_bytes.len()
}

/// Intended to go from the last character of a span to the nearest whitespace so that it
/// doesn't point to the entire line. If a string happens to have whitespace it will just ignore
/// it which could change.
fn get_err_start(src: &[u8], span_end: usize) -> usize {
    let line_start = get_start_of_line(src, span_end);

    for i in (line_start..span_end).rev() {
        let b = src[i - 1];
        if b == b' ' || b == b'\t' || b == b'\r' {
            return i;
        }
    }

    line_start
}

fn char_width_offset(src_str: &str, start: usize, end: usize) -> usize {
    // let ws_amt = src_str[start..end]
    //     .chars()
    //     .rev()
    //     .take_while(|c| c.is_whitespace())
    //     .count();

    // dbg!(ws_amt);
    src_str[start..end]
        .chars()
        .map(|c| UnicodeWidthChar::width(c).unwrap_or(1))
        .sum()
}

//TODO: Needs better way to get color data and highlight type
/// Formats a single line segment with arrows under the error span.
fn format_line_segment(
    ln_num: usize,
    ln_str: &str,
    ln_span_start: usize,
    ln_span_end: usize,
    can_color: bool,
) -> String {
    // Is zero and reusing offset since it's only the space
    let space_offset = char_width_offset(ln_str, 0, ln_span_start);

    //WARN: If this isn't done EOF cases cause a panic.
    // May just no have EOF spanning displayed but only if bugs are present from this.
    let end = if ln_span_end + 1 > ln_str.len() {
        ln_span_end
    } else {
        // ln_span_end is + 1 due to the spans from the lexer producing inclusive, exclusive ranges.
        ln_span_end + 1
    };

    let arrow_offset = char_width_offset(ln_str, ln_span_start, end);

    let spaces = " ".repeat(space_offset);
    let arrows = "^".repeat(arrow_offset);

    let mut i = ln_num;
    let mut ln_num_size = 0;

    // Calculating size of given line number for '|' alignment
    // I think this is ok?
    while i / 10 != 0 {
        ln_num_size += 1;
        i /= 10;
    }

    let bar_spacing = " ".repeat(ln_num_size);

    if can_color {
        format!(" {bar_spacing}|\n{ln_num}|\t{ln_str}\n {bar_spacing}|\t{spaces}{RED}{arrows}{NC}")
    } else {
        format!(" {bar_spacing}|\n{ln_num}|\t{ln_str}\n {bar_spacing}|\t{spaces}{arrows}")
    }
}
