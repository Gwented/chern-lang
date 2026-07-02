use std::{collections::HashSet, ops::RangeInclusive};
//WARN: Handles EOF in a possibly weird manner

use unicode_width::UnicodeWidthChar;

use crate::{id_types::SourceRegionId, source_map::source_span::SourceSpan};

/// High level struct of all byte and line number data for each line inside of it
#[derive(Debug)]
pub struct LineView {
    /// SourceSpan of the start and end line within `lines`. This represents high level line
    /// numbers not any sort of source byte spanning.
    pub ln_num_range: RangeInclusive<u32>,
    /// Detailed lines
    pub lines: Vec<Line>,
    pub region_id: SourceRegionId,
}

/// Basic line structure for metadata
#[derive(Debug)]
pub struct Line {
    pub ln_num: u32,
    pub ln_span: SourceSpan,
}

/// Helper struct that stores a line number and spans associated with the line number
#[derive(Debug)]
struct LineGroup {
    ln_num: u32,
    spans: Vec<SourceSpan>,
}

impl LineGroup {
    fn new(ln_num: u32, spans: Vec<SourceSpan>) -> LineGroup {
        LineGroup { ln_num, spans }
    }
}

/// Helper struct for using methods for indexing `LineGroup` as a HashMap without allocating for a
/// HashMap
#[derive(Debug)]
pub struct LineGroupManager {
    // For vectors on the same line to be grouped in the same diagnostic
    span_groups: Vec<LineGroup>,
}

impl LineGroupManager {
    fn new() -> LineGroupManager {
        LineGroupManager {
            span_groups: Vec::new(),
        }
    }

    /// Inserts and immediately sorts the given span within it's correct line vector.
    /// This method also ensures no duplicates are stored.
    fn insert(&mut self, ln_key: u32, span: &SourceSpan) {
        // Checking if the line key already exists before making a new pair
        if let Some(pair) = self
            .span_groups
            .iter_mut()
            .find(|group| group.ln_num as u32 == ln_key)
        {
            //FIX: Do not insert duplicates
            if pair.spans.iter().any(|s| s == span) {
                return;
            };

            pair.spans.push(span.clone());

            pair.spans.sort_by_key(|s| s.start);
        } else {
            self.span_groups
                .push(LineGroup::new(ln_key as u32, vec![*span]));
        }
    }

    /// Removes any remaining overlapping spans so that the last_span_start math does not exhibit
    /// undefined behavior
    fn curate(&mut self) {
        // No
        let mut removable_indices: HashSet<usize> = HashSet::new();
        for span_group in self.span_groups.iter_mut().map(|group| &mut group.spans) {
            for i in 0..span_group.len() {
                for j in 0..span_group.len() {
                    if j < span_group.len() && span_group[i].contains(span_group[j]) {
                        if j == i {
                            continue;
                        }

                        removable_indices.insert(j);
                    }
                }
            }

            // I know. I know.
            // I. Know.
            if !removable_indices.is_empty() {
                let mut filtered_group: Vec<SourceSpan> = Vec::new();

                for (i, span) in span_group.iter().enumerate() {
                    if removable_indices.contains(&i) {
                        continue;
                    }

                    filtered_group.push(*span);
                }

                *span_group = filtered_group;

                removable_indices.clear();
            }
        }
    }
}

// Ability to choose color when help exists in a better form
/// Returns the line number, column and red arrows under given spans
pub fn something(src_bytes: &[u8], span: &SourceSpan) -> LineGroupManager {
    // // So it doesn't just explode upon no spans given since diagnostics are not essential to the
    // // program actually emitting other diagnostics. Could also just turn this into Option
    let ln_view = form_ln_view(src_bytes, span);

    // --FIRST--
    // Forming data about every line in the given span so it can be mutated or used in a way that
    // is non-linear, which was a persistent issue with past designs. It is here to offer a high
    // level view.

    // External use: This is only needed if a given span could exceed one line.
    // --SECOND--
    // Curating spans to ensure all spans that may exceed their line are properly cut for their
    // line so later formatting is not made more complicated
    let mut curated_spans: Vec<SourceSpan> = Vec::new();
    //
    // //Maybe can be handled better
    // for ln in &ln_view.lines {
    //     let ln_range = ln.ln_span.start..=ln.ln_span.end;
    //     for span in spans {
    //         // If the line does not contain the span then it is skipped.
    //         // The curate span function would form the entire line by default without this check.
    //         if span.start > *ln_range.end() || *ln_range.start() > span.end {
    //             // dbg!(&ln_range, span);
    //             continue;
    //         }
    //
    //         curated_spans.push(curate_span(&ln, span));
    //     }
    // }
    // // dbg!(curated_spans, spans);
    // // panic!();
    //
    // // --THIRD--
    // // Putting all spans into a key-value pair so that they can have their errors reported in
    // // groups. This is to avoid the issue of span print duplicates.
    // let mut ln_groups: LineGroups = LineGroups::new();
    //
    // for (i, ln_span) in ln_view.lines.iter().map(|ln| &ln.ln_span).enumerate() {
    //     let range = ln_span.start..=ln_span.end;
    //
    //     for span in &curated_spans {
    //         if range.contains(&span.start) || range.contains(&span.end) {
    //             let ln_key = ln_view.lines[i].ln_num;
    //             ln_groups.insert(ln_key, span);
    //         }
    //     }
    // }
    //
    // // Removes any remaining overlapping spans
    // // This is required due to how the last_span_start variable behaves
    // ln_groups.curate();
    //
    // // --FOURTH--
    // // Giving each group their own diagnostic
    // let mut fmtted_diags: Vec<String> = Vec::new();
    // let src_str = str::from_utf8(src_bytes).unwrap_or("<invalid UTF-8 in source file>");
    //
    // // Getting the largest number to see if the entire print should align with a bigger spacing
    // let ln_num_width = get_num_width(ln_view.ln_num_span.end);
    //
    // for ln in &ln_view.lines {
    //     for group in &ln_groups.span_groups {
    //         if ln.ln_num == group.0 {
    //             let diag = form_ln_diag(src_str, ln, ln_num_width, &group.1, can_color);
    //             fmtted_diags.push(diag);
    //         }
    //     }
    // }
    //
    // // -- FINAL --
    // // Taking what this particular error message handler wants to display out of the fully made
    // // error messages.
    //
    // let mut final_diag = String::new();
    //
    // // Will maybe just create this earlier so 2 strings aren't alloced
    // let num_spaces = " ".repeat(ln_num_width);
    //
    // // Bit of a weird way to output desired formatting
    // final_diag.push_str(&format!("{num_spaces}|"));
    // final_diag.push_str(&fmtted_diags[0]);
    //
    // if ln_view.ln_num_span.end - ln_view.ln_num_span.start >= 2 {
    //     let dash_spaces = " ".repeat(ln_num_width - 1);
    //
    //     final_diag.push_str(&format!("\n{dash_spaces}---"));
    //     final_diag.push_str(&fmtted_diags[fmtted_diags.len() - 1]);
    // } else if fmtted_diags.len() == 2 {
    //     final_diag.push_str(&fmtted_diags[1]);
    // }
    //
    // let eof_byte_pos = src_bytes.len() - 1;
    // // The length of diagnostics is checked too because the line view picks up empty lines but
    // // ignores eof. If it didn't check the lenght of diagnostics then "EOF" would be awkwardly
    // // shoved into the diagnostics.
    // if eof_byte_pos == spans[spans.len() - 1].end && fmtted_diags.len() == 1 {
    //     final_diag.push_str("\nEOF");
    // }
    //
    // // Is + 1 because columns count starting by 1
    // let col = get_chars_width(src_str, actual_start, start) + 1;
    //
    // LineData {
    //     diag: final_diag,
    //     ln: ln_view.ln_num_span.start,
    //     col,
    // }
    todo!()
}

//TODO: This, but diagnostics have a set of special instructions that display this graphic.
//So, there would be an "add_graphic(AnnotationGraphic::HelpTransform(args))" where the args are
//specific to the enum. The renderer can just decide if graphics should be ignored.
/// Error message type:
/// X -> X()
///      +++
// The X should be red and the right X should have green + signs under the params
// This is specific right now but will turn to more generic just pointing to transformation
// Maybe prefix?
pub fn help_transform(from: &str, to: &str, can_color: bool) -> String {
    todo!()
    // let (red, nc) = color::get_red(can_color);
    // let (green, _) = color::get_green(can_color);
    //
    // let from_spaces = " ".repeat(UnicodeWidthStr::width(from));
    //
    // let arrow = format!(" -> ");
    // let arrow_spaces = " ".repeat(arrow.len());
    //
    // let fmtted_to = format!("{green}{to}{nc}");
    //
    // let to_width = UnicodeWidthStr::width(to);
    //
    // let add_amt = "+".repeat(to_width);
    // let add = format!("{green}{add_amt}{nc}");
    //
    // let diag = format!("\t{red}{from}{nc}{arrow}{fmtted_to}\n\t{from_spaces}{arrow_spaces}{add}");
    //
    // diag
}

/// Checks if given span is on the same line, and if it is not the span is adjusted to fit the line.
/// Always returns a new span which could potentially be altered.
fn curate_span(ln: &Line, span: &SourceSpan) -> SourceSpan {
    todo!()
    // let ln_range = ln.ln_span.start..=ln.ln_span.end;
    //
    // if !ln_range.contains(&span.start) && !ln_range.contains(&span.end) {
    //     return SourceSpan::new(ln.ln_span.start, ln.ln_span.end);
    // } else if !ln_range.contains(&span.start) {
    //     return SourceSpan::new(ln.ln_span.start, span.end);
    // } else if !ln_range.contains(&span.end) {
    //     return SourceSpan::new(span.start, ln.ln_span.end);
    // };
    //
    // span.clone()
}

/// Goes from the start to the end of the span collecting all line data so that any sort of later
/// complex error handling does not need any re-computation, and has a high level view of all lines
/// in the given span. New lines are never in a line's span unless the line only contains a single
/// new line.
//WARN: EOF's final byte is not currently detected
pub fn form_ln_view(src_bytes: &[u8], span: &SourceSpan) -> LineView {
    // Getting the first line's start position since span.start could start later in the actual
    // line. May make this something that just needs to be done outside.

    let span_start = span.start as usize;
    let span_end = span.end as usize;

    let actual_span_start = get_ln_start_byte(src_bytes, span_start);

    let full_span = SourceSpan::new(span.region_id, actual_span_start as u32, span.end);

    let first_ln_start = actual_span_start;

    let mut i = first_ln_start;

    let mut lines: Vec<Line> = Vec::new();

    // Decoupled for readability. Current start is technically is just i.
    // Every i is not a current start, but every current start is i + 1 or 2.
    let mut current_start = first_ln_start;

    let first_ln_num = get_ln_num(src_bytes, actual_span_start) as u32;

    // To assign a line number to all processed lines
    let mut current_ln_num = first_ln_num;
    //TODO: Change to start and len so that an eof byte is implicitly tracked as opposed to
    //depending on new lines to end

    //NOTE: Uses the first_ln_start as the default first line, then goes through every line within the
    // given span until it reaches the end of the span, collecting all `Line` information.
    // These are structured to be (inclusive, inclusive)
    // `current_start` positions itself at the first line of the next line.
    // `current_end` assumes the current start is already set, and positions itself at wherever the
    // last line would've ended.
    while i < src_bytes.len() {
        let b = src_bytes[i];

        //TODO: CHECK WINDOWS
        if b == b'\r' && src_bytes.get(i + 1) == Some(&b'\n') {
            // if the previous byte was a \n then that means this line is a singular new line and
            // line start == line end, otherwise the actual end is - 1
            //
            // Same eof byte pos reasoning as '\n' below
            let current_end = if src_bytes.get(i - 1) == Some(&b'\n') {
                i
            } else {
                // Still i - 1 here since the carriage return is stopped at and both are skipped at
                // once in the end.
                i - 1
            } as u32;

            let ln = Line {
                ln_num: current_ln_num,
                ln_span: SourceSpan::new(span.region_id, current_start as u32, current_end),
            };

            lines.push(ln);

            // To avoid reading entire file
            if i > span_end {
                break;
            }

            current_start = i + 2;

            current_ln_num += 1;
            i += 2;
        } else if b == b'\n' {
            // Processes single new line line as a singular line with one '\n' inside.
            // This is so all lines are accounted for empty or not. No particular reason for this
            // to happen but it is done just in case.
            //
            // This needs an OR case because if i == eof then we want to preserve the new line byte
            // for source retention purposes. So, this is an accepted heuristic tooling likely will
            // need to just account for.
            let current_end = if src_bytes.get(i - 1) == Some(&b'\n') {
                i
            } else {
                i - 1
            };

            let ln = Line {
                ln_num: current_ln_num,
                ln_span: SourceSpan::new(span.region_id, current_start as u32, current_end as u32),
            };

            lines.push(ln);

            if i > span_end {
                break;
            }

            current_start = i + 1;

            current_ln_num += 1;
            i += 1;
        } else {
            // Incrementing forward normally if no new line bytes
            i += 1;
        }
    }

    // WARN: This seemingly works fine
    // lex_tok_test_rev causes this error
    //
    // This exists because the main loop above ONLY processes a line if it seens a new line after
    // starting, meaning a single line, with no new line, is completely ignored.
    if lines.is_empty() {
        let only_ln = Line {
            ln_num: first_ln_num,
            ln_span: full_span,
        };

        lines.push(only_ln);
    }

    let eof_byte_pos = src_bytes.len() - 1;
    let span_ends_at_eof = eof_byte_pos == span_end;

    // Current start is a variable that eagerly goes to the start of the next line. Meaning, if it
    // reaches the last line, it should be greater than the actual end since it should be at what
    // would be the next line since goes to the next line's position whether or not it exists.
    //
    // So if eof_byte > current_start, that means the last line wasn't processed, which only
    // happens if an abrupt end occurs, which is only caused by `@end` since it's the only case
    // where the eof byte would not just be the last byte.
    if span_ends_at_eof && eof_byte_pos > current_start {
        // The main loop does not detect the line and end
        let end_ln = Line {
            ln_num: current_ln_num,
            ln_span: SourceSpan::new(span.region_id, current_start as u32, eof_byte_pos as u32),
        };

        lines.push(end_ln);
        // Mutates the last line so that it captures eof since it skips it otherwise
    } else if span_ends_at_eof {
        let last_pos = lines.len() - 1;
        lines[last_pos].ln_span.end = eof_byte_pos as u32;
    }

    let ln_num_range =
        RangeInclusive::new(first_ln_num as u32, lines[lines.len() - 1].ln_num as u32);

    LineView {
        ln_num_range,
        lines,
        region_id: span.region_id,
    }
}

/// Gets the first new line byte using the given position
fn get_ln_start_byte(src_bytes: &[u8], pos: usize) -> usize {
    // If there is an out of bounds error here, it is possible that module reporting was done
    // wrongly elsewhere
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
    grouped_spans: &[SourceSpan],
    // color:
    // pointer_type: char
    can_color: bool,
) -> String {
    todo!()
    // let mut plain_ln = String::new();
    //
    // // Lines containing only a new line are empty lines that were found by the line view function.
    // // This prevents formatting errors by just ignoring new line only lines
    // if src_str.as_bytes()[ln.ln_span.start] != b'\n' {
    //     // Is an inclusive range since spans are inclusive, inclusive
    //     plain_ln.push_str(&src_str[ln.ln_span.start..=ln.ln_span.end]);
    // }
    //
    // let (red, nc) = color::get_red(can_color);
    //
    // // The tip arrows under the plain line
    // let mut pointers = String::new();
    // let mut last_span_start = ln.ln_span.start;
    //
    // // let eof_byte_pos = src_str.as_bytes().len() - 1;
    //
    // for span in grouped_spans {
    //     // dbg!(ln, grouped_spans);
    //     // dbg!(span);
    //     // //  TEST: EOF
    //     // if last_span_start == eof_byte_pos {
    //     //     break;
    //     // }
    //
    //     debug_assert!(
    //         last_span_start <= span.end,
    //         "'form_ln_diag' failed diagnostic spanning. last_span_start: {} <= span.end: {}",
    //         last_span_start,
    //         span.end
    //     );
    //
    //     let space_count = get_chars_width(src_str, last_span_start, span.start);
    //
    //     // Space count added makes last_span_start one before the actual span. The difference of
    //     // span end + 1 and span start is the actual span that needs to be skipped.
    //     last_span_start += space_count + ((span.end + 1) - span.start);
    //
    //     pointers.push_str(&" ".repeat(space_count));
    //
    //     // Since the function is inclusive, exclusive a + 1 is needed
    //     let arrow_count = get_chars_width(src_str, span.start, span.end + 1);
    //     let arrows = "^".repeat(arrow_count);
    //     let colored_arrows = format!("{red}{arrows}{nc}");
    //
    //     pointers.push_str(&colored_arrows);
    // }
    //
    // //TEST:
    // // if eof_byte_pos == grouped_spans[grouped_spans.len() - 1].end {
    // //     return "".to_string();
    // // }
    //
    // let current_ln_num_size = get_num_width(ln.ln_num);
    // let bar_spaces = " ".repeat(ln_num_width);
    // // Ensuring that the size of the current number aligns with the vertical bars
    // let num_alignment = " ".repeat(ln_num_width - current_ln_num_size);
    //
    // let fmtted_ln_num = format!("{}{num_alignment}", ln.ln_num);
    //
    // let diag = format!("\n{fmtted_ln_num}|\t{plain_ln}\n{bar_spaces}|\t{pointers}");
    //
    // diag
}

// _Generic?
/// Is the preferred function for getting number widths to avoid allocating strings just for number sizes
pub fn get_num_width(num: usize) -> usize {
    let mut size = 0;
    let mut i = num;

    while i != 0 {
        i /= 10;
        size += 1;
    }

    size
}

/// Returns character width count within the given start and end (inclusive, exclusive)
pub fn get_chars_width(s: &str, start: usize, end: usize) -> usize {
    // if start > end {
    //     dbg!(&s[end..=start]);
    // }
    s[start..end]
        .chars()
        .map(|c| UnicodeWidthChar::width(c).unwrap_or(1))
        .sum()
}
