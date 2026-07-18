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
            .find(|group| group.ln_num == ln_key)
        {
            //FIX: Do not insert duplicates
            if pair.spans.iter().any(|s| s == span) {
                return;
            };

            pair.spans.push(*span);

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

    // dbg!(span_start, span_end);
    // panic!();

    // dbg!(str::from_utf8(src_bytes), span);
    // panic!();
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

    //WARN: CHANGED TO INCLUSIVE EXCLUSIVE
    //NOTE: Uses the first_ln_start as the default first line, then goes through every line within the
    // given span until it reaches the end of the span, collecting all `Line` information.
    // These are structured to be (inclusive, exclusive)
    // `current_start` positions itself at the first line of the next line.
    // `current_end` assumes the current start is already set, and positions itself at wherever the
    // last line would've ended.
    while i < src_bytes.len() {
        let b = src_bytes[i];

        if b == b'\r' && src_bytes.get(i + 1) == Some(&b'\n') {
            // if the previous byte was a \n then that means this line is a singular new line and
            // line start == line end, otherwise the actual end is - 1
            //
            // Same eof byte pos reasoning as '\n' below
            let current_end = if src_bytes.get(i - 1) == Some(&b'\n') {
                // i
                i + 1
            } else {
                // Still i here since the carriage return is stopped at and both are skipped at
                // once in the end.
                // i - 1
                i
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
                // i
                i + 1
            } else {
                // i - 1
                i
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
    // let span_ends_at_eof = eof_byte_pos == span_end;
    //WARN: Is this right?
    let span_ends_at_eof = span_end == src_bytes.len();

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
            // ln_span: SourceSpan::new(span.region_id, current_start as u32, eof_byte_pos as u32),
            ln_span: SourceSpan::new(
                span.region_id,
                current_start as u32,
                (eof_byte_pos + 1) as u32,
            ),
        };

        lines.push(end_ln);
        // Mutates the last line so that it captures eof since it skips it otherwise
    } else if span_ends_at_eof {
        let last_pos = lines.len() - 1;
        // lines[last_pos].ln_span.end = eof_byte_pos as u32;
        lines[last_pos].ln_span.end = (eof_byte_pos + 1) as u32;
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
