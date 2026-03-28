//TODO: ORGANIZE NEW ARCHITECTURE
// MAKE PARAMS ONLY TAKE SPAN SINCE LINES ARE BUILT ANYWAYS
use unicode_width::UnicodeWidthChar;

use crate::{color, symbols::Span};

const TOTAL_SEPARATORS: usize = 60;

pub struct LineData {
    diag: String,
    ln: usize,
    col: usize,
}

// LineCache held by context?

// Holds the first line, all lines from the start span to the end span's metadata
#[derive(Debug)]
struct LineView {
    // Span contain the lines themselves not any byte offset
    ln_num_span: Span,
    // Sorted by default
    lines: Vec<Line>,
}

/// Basic line structure for metadata
#[derive(Debug)]
struct Line {
    ln_num: usize,
    ln_span: Span,
}

/// Wrapper for methods instead of in-lining or making another structure to depict the key-value
/// mapping of spans so that they can be grouped for diagnostics.
#[derive(Debug)]
struct LineGroups {
    // For vectors on the same line to be grouped in the same diagnostic
    span_groups: Vec<(usize, Vec<Span>)>,
}

// Maybe this isn't needed since there is no realistically huge loss from doing any of this
// pub struct LineCache {
//     lines: Vec<Line>,
// }

impl LineGroups {
    fn new() -> LineGroups {
        LineGroups {
            span_groups: Vec::new(),
        }
    }

    /// Inserts and immediately sorts the given span within it's correct line vector.
    /// This method also ensures no duplicates are stored
    fn insert(&mut self, ln_key: usize, span: &Span) {
        if let Some(pair) = self.span_groups.iter_mut().find(|group| group.0 == ln_key) {
            //FIX: Do not insert duplicates
            if pair.1.iter().any(|s| s == span) {
                return;
            };

            pair.1.push(span.clone());

            pair.1.sort_by_key(|s| s.start);
        } else {
            self.span_groups.push((ln_key, vec![span.clone()]));
        }
    }
}

// Ability to choose color when help exists in a better form
/// Returns the line number, column and red arrows under given spans
pub fn form_err_diag(src_bytes: &[u8], spans: &[Span], can_color: bool) -> LineData {
    let start = spans.iter().map(|s| s.start).min().expect("Cannot be < 1");
    let actual_start = get_ln_start_byte(src_bytes, start);

    let end = spans.iter().map(|s| s.end).max().expect("Cannot be < 1");

    let full_span = Span::new(actual_start, end);

    // --FIRST--
    // Forming data about every line in the given span so it can be mutated or used in a way that
    // is non-linear, which was a persistent issue with past designs. It is here to offer a high
    // level view.
    let ln_view = form_ln_view(src_bytes, &full_span);

    // --SECOND--
    // Curating spans to ensure all spans that may exceed their line are properly cut for their
    // line so later formatting is not made more complicated
    let mut curated_spans: Vec<Span> = Vec::new();

    //Maybe can be handled better
    for ln in &ln_view.lines {
        let range = ln.ln_span.start..=ln.ln_span.end;
        for span in spans {
            // Checking if the line actually has the span which would otherwise push entire lines
            // by default.
            if !range.contains(&span.start) && !range.contains(&span.end) {
                continue;
            }

            curated_spans.push(curate_span(&ln, span));
        }
    }

    // --THIRD--
    // Putting all spans into a key-value pair so that they can have their errors reported in
    // groups. This is to avoid the persistent issue of span print duplicates.
    let mut ln_groups: LineGroups = LineGroups::new();

    //FIXME: OR HERE
    for (i, ln_span) in ln_view.lines.iter().map(|ln| &ln.ln_span).enumerate() {
        let range = ln_span.start..=ln_span.end;

        for span in &curated_spans {
            if range.contains(&span.start) || range.contains(&span.end) {
                let ln_key = ln_view.lines[i].ln_num;
                ln_groups.insert(ln_key, span);
            }
        }
    }

    //FIXME: FILTER HERE IN-CASE OF DUPLICATES

    dbg!(&ln_groups);
    // panic!();

    // --FOURTH--
    // Giving each group their own diagnostic
    let mut fmtted_diags: Vec<String> = Vec::new();
    let src_str = str::from_utf8(src_bytes).unwrap_or("<invalid UTF-8 in source file>");

    // Getting the largest number to see if the entire print should align with a bigger spacing
    let ln_num_width = get_num_width(ln_view.ln_num_span.end);

    for ln in &ln_view.lines {
        for group in &ln_groups.span_groups {
            if ln.ln_num == group.0 {
                let diag = form_ln_diag(src_str, ln, ln_num_width, &group.1, can_color);
                fmtted_diags.push(diag);
            }
        }
    }

    // -- FINAL --
    // Taking what this particular error message handler wants to display out of the fully made
    // error messages.

    //TODO:

    let mut final_diag = String::new();

    // Will maybe just create this earlier so 2 strings aren't alloced
    let num_spaces = " ".repeat(ln_num_width);

    final_diag.push_str(&format!("{num_spaces}|"));
    final_diag.push_str(&fmtted_diags[0]);

    // Will need the dashes to be dependent on existing line metadata
    if ln_view.ln_num_span.end - ln_view.ln_num_span.start >= 2 {
        let dash_spaces = " ".repeat(ln_num_width - 1);

        final_diag.push_str(&format!("\n{dash_spaces}---"));
        final_diag.push_str(&fmtted_diags[fmtted_diags.len() - 1]);
    } else if fmtted_diags.len() == 2 {
        final_diag.push_str(&fmtted_diags[1]);
    }

    // Is + 1 because columns count starting by 1
    let col = get_chars_width(src_str, actual_start, start) + 1;

    LineData {
        diag: final_diag,
        ln: ln_view.ln_num_span.start,
        col,
    }
}

/// Checks if given span is on the same line, and if it is not the span is adjusted to fit the line.
/// Always returns a new span which could potentially be altered.
fn curate_span(ln: &Line, span: &Span) -> Span {
    let ln_range = ln.ln_span.start..=ln.ln_span.end;

    if !ln_range.contains(&span.start) && !ln_range.contains(&span.end) {
        return Span::new(ln.ln_span.start, ln.ln_span.end);
    } else if !ln_range.contains(&span.start) {
        return Span::new(ln.ln_span.start, span.end);
    } else if !ln_range.contains(&span.end) {
        return Span::new(span.start, ln.ln_span.end);
    };

    span.clone()
}

/// Goes from the start to the end of the span collecting all line data so that any sort of later
/// complex error handling does not need any re-computation, and has a high level view of all lines
/// in the given span.
fn form_ln_view(src_bytes: &[u8], span: &Span) -> LineView {
    // Getting the first line's start position since span.start could start later in the actual
    // line. May make this something that just needs to be done outside.
    let first_ln_start = span.start;

    let mut i = first_ln_start;

    let mut lines: Vec<Line> = Vec::new();

    // Decoupled for readability. Current start is technically is just i.
    // Every i is not a current start, but every current start is i + 1 or 2.
    let mut current_start = first_ln_start;

    let ln_start = get_ln_num(src_bytes, span.start);

    // To assign a line number to all processed lines
    let mut current_ln_num = ln_start;

    //NOTE: Uses the first_ln_start as the default first line, then goes through every line within the
    // given span until it reaches the end of the span, collecting all `Line` information.
    // These are structured to be (inclusive, inclusive)
    // `current_start` positions itself at the first line of the next line.
    // `current_end` assumes the current start is already set, and positions itself at wherever the
    // last line would've ended.
    while i < src_bytes.len() {
        let b = src_bytes[i];

        if b == b'\r' && src_bytes.get(i + 1) == Some(&b'\n') {
            // if the previous byte was a \n then that means this line is a singular new line and
            // line start == line end, otherwise the actual end is - 2
            let current_end = if src_bytes.get(i - 1) == Some(&b'\n') {
                i
            } else {
                // Still i - 1 here since the carriage return is stopped at and both are skipped at
                // once in the end.
                i - 1
            };

            // Could also collect the line it's on but that data is not important here
            let ln = Line {
                ln_num: current_ln_num,
                ln_span: Span::new(current_start, current_end),
            };

            lines.push(ln);

            // To avoid reading entire file
            if i > span.end {
                break;
            }

            current_start = i + 2;

            current_ln_num += 1;
            i += 2;
        } else if b == b'\n' {
            // Processes single new line line as a singular line with one '\n' inside.
            // This is so all lines are accounted for empty or not. Not particular reason for this
            // to happen but it is done just in case.
            let current_end = if src_bytes.get(i - 1) == Some(&b'\n') {
                i
            } else {
                i - 1
            };

            let ln = Line {
                ln_num: current_ln_num,
                ln_span: Span::new(current_start, current_end),
            };

            lines.push(ln);

            if i > span.end {
                break;
            }

            current_start = i + 1;

            current_ln_num += 1;
            i += 1;
        } else {
            i += 1;
        }

        // WARN: TEMP EOF '@end' EDGE CASE PRINTING
        if i == span.end && span.end == src_bytes.len() {
            let current_end = i - 1;

            let ln = Line {
                ln_num: current_ln_num,
                ln_span: Span::new(current_start, current_end),
            };

            lines.push(ln);

            break;
        }
    }

    let ln_span = Span::new(ln_start, current_ln_num);
    LineView {
        ln_num_span: ln_span,
        lines,
    }
}

/// Gets the first new line byte using the given position
fn get_ln_start_byte(src_bytes: &[u8], pos: usize) -> usize {
    for i in (0..=pos).rev() {
        let b = src_bytes[i];

        if b == b'\n' {
            return i + 1;
        }
    }

    // Returns zero so that it's still returning the start of the line even at the beginning of the
    // file
    0
}

/// Get's the line number of the given start byte position
fn get_ln_num(src_bytes: &[u8], start: usize) -> usize {
    // Could be issues here regarding line numbers being counted for single line lines
    let mut ln_num = 1;
    for i in 0..=start {
        let b = src_bytes[i];

        if b == b'\n' {
            ln_num += 1;
        }
    }

    ln_num
}

/// Forms a diagnostic with arrows under the grouped spans.
/// Assumes the given line group has it's spans stored on the same relevant line using
/// `LineGroups`. Assumes the given line group is also sorted. Also assumes the spans don't span
/// past their respective line. This cannot be used unless all previous steps are done.
fn form_ln_diag(
    src_str: &str,
    ln: &Line,
    ln_num_width: usize,
    grouped_spans: &Vec<Span>,
    can_color: bool,
) -> String {
    // Maybe separating this basic line from the formatted arrows could be better later
    let mut plain_ln = String::new();

    // Is an inclusive range since spans are inclusive, exclusive
    plain_ln.push_str(&src_str[ln.ln_span.start..=ln.ln_span.end]);

    let (red, nc) = color::get_red(can_color);

    // The tip arrows under the plain line
    let mut pointers = String::new();
    let mut last_span_start = ln.ln_span.start;

    for span in grouped_spans {
        let space_count = get_chars_width(src_str, last_span_start, span.start);

        // Space count added makes last_span_start one before the actual span. The difference of
        // span end + 1 and span start is the actual span that needs to be skipped.
        last_span_start += space_count + ((span.end + 1) - span.start);

        pointers.push_str(&" ".repeat(space_count));

        // Since the function is inclusive, exclusive a + 1 is needed
        let arrow_count = get_chars_width(src_str, span.start, span.end + 1);
        let arrows = "^".repeat(arrow_count);
        let colored_arrows = format!("{red}{arrows}{nc}");

        pointers.push_str(&colored_arrows);
    }

    let current_ln_num_size = get_num_width(ln.ln_num);
    let bar_spaces = " ".repeat(ln_num_width);
    // Ensuring that the size of the current number aligns with the vertical bars
    let num_alignment = " ".repeat(ln_num_width - current_ln_num_size);

    let fmtted_ln_num = format!("{}{num_alignment}", ln.ln_num);

    let diag = format!("\n{fmtted_ln_num}|\t{plain_ln}\n{bar_spaces}|\t{pointers}");

    diag
}

pub fn standardize_err(base_msg: &str, line_data: &LineData, help: &str) -> String {
    format!(
        "{base_msg}\n[{}:{}]\n{}\n{help}{}",
        line_data.ln,
        line_data.col,
        line_data.diag,
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

/// Is the preferred function for getting number widths to avoid allocating strings just for number sizes
fn get_num_width(num: usize) -> usize {
    let mut size = 1;
    let mut i = num;

    while i > 10 {
        i /= num;
        size += 1;
    }

    size
}

/// Returns character width count within the given start and end (inclusive, exclusive)
fn get_chars_width(s: &str, start: usize, end: usize) -> usize {
    if end > start {
        dbg!(&s[start..end]);
    }

    s[start..end]
        .chars()
        .map(|c| UnicodeWidthChar::width(c).unwrap_or(1))
        .sum()
}
