//! # text
//!
//! UTF-8 ↔ LSP UTF-16 position conversion utilities and incremental text-change application.
//!
//! ## UTF-16 encoding requirement
//!
//! The LSP specification requires that line/character positions use **UTF-16 code
//! units** as the character unit.  Chern source files are stored internally as UTF-8,
//! so all functions in this module perform the necessary conversion.
//!
//! Most ASCII `.chrn` files will not contain multi-byte characters, so the conversion
//! overhead is negligible.  Files containing emoji or CJK characters (e.g. in string
//! literals or comments) will diverge from a naïve byte-count approach.
//!
//! ## Word boundary definition
//!
//! The functions [`extract_word_at`] and [`find_word_bounds`] use an extended
//! definition of "word character" that includes `_`, `#`, `@`, `<`, `>`, and `-`
//! in addition to alphanumeric characters.  This matches how the Chern lexer
//! tokenises identifiers and directives such as `@def`, `#warn`, and `var->`.

use chrn_utils::source_map::source_span::SourceSpan;
use std::collections::HashMap;
use std::sync::Arc;
use tower_lsp::lsp_types::Position;
use tower_lsp::lsp_types::Range;
use tower_lsp::lsp_types::TextDocumentContentChangeEvent;

fn is_word_char(c: u8) -> bool {
    let c = c as char;
    c.is_alphanumeric() || c == '_' || c == '#' || c == '@' || c == '<' || c == '>' || c == '-'
}

/// Extracts the word that contains position `idx` from `line`.
///
/// Uses the extended word-character set (alphanumeric plus `_#@<>-`).
/// `idx` is a **character** (byte) index into `line`; it is clamped to `line.len()`.
pub fn extract_word_at(line: &str, idx: usize) -> String {
    let idx = idx.min(line.len());
    let bytes = line.as_bytes();
    let mut start = idx;
    while start > 0 && is_word_char(bytes[start - 1]) {
        start -= 1;
    }
    let mut end = idx;
    while end < bytes.len() && is_word_char(bytes[end]) {
        end += 1;
    }

    line[start..end].to_string()
}

/// Applies an incremental or full text change to an existing document string.
///
/// # Parameters
/// * `existing` — The current document text.
/// * `change`   — An LSP `TextDocumentContentChangeEvent`.
///
/// # Returns
/// * `Ok(String)` containing the updated document text.
/// * `Err(String)` when the change range extends beyond the document boundaries
///   (the caller may fall back to a full replacement in this case).
///
/// # Full replacement
/// When `change.range` is `None` the client replaces the entire document with
/// `change.text` and the function returns `Ok(change.text.clone())`.
pub fn apply_text_change(
    existing: &str,
    change: &TextDocumentContentChangeEvent,
) -> Result<String, String> {
    // If no range is provided, the client replaced the entire document
    let Some(range) = change.range else {
        return Ok(change.text.clone());
    };

    let start = position_to_offset(existing, range.start);
    let end = position_to_offset(existing, range.end);
    if start > existing.len() || end > existing.len() || start > end {
        // Fallback: treat as full replacement
        return Err(format!(
            "invalid range start={} end={} len={}",
            start,
            end,
            existing.len()
        ));
    }

    let mut out = String::with_capacity(existing.len() - (end - start) + change.text.len());
    out.push_str(&existing[..start]);
    out.push_str(&change.text);
    out.push_str(&existing[end..]);
    Ok(out)
}

/// Converts an LSP [`Position`] (line, UTF-16 character) to a UTF-8 byte offset.
///
/// If `pos.line` is beyond the last line, `text.len()` is returned.
/// If `pos.character` is beyond the end of the requested line, the byte offset
/// of the end of that line (including the newline) is returned.
pub fn position_to_offset(text: &str, pos: Position) -> usize {
    let mut offset = 0;
    for (line, ln) in text.split_inclusive('\n').enumerate() {
        if line as u32 == pos.line {
            let mut current_utf16_idx = 0;
            for (byte_idx, c) in ln.char_indices() {
                if current_utf16_idx >= pos.character as usize {
                    return offset + byte_idx;
                }
                current_utf16_idx += c.len_utf16();
            }
            // If requested character past line end, return end of this line
            return offset + ln.len();
        }
        offset += ln.len();
    }

    // If line beyond text, return len
    text.len()
}

/// Find the byte bounds (start, end) of the word containing the given byte offset.
pub fn find_word_bounds(text: &str, offset: usize) -> (usize, usize) {
    let bytes = text.as_bytes();
    let len = bytes.len();
    if len == 0 {
        return (0, 0);
    }
    let mut start = offset.min(len);
    while start > 0 && is_word_char(bytes[start - 1]) {
        start -= 1;
    }

    let mut end = offset.min(len);
    while end < len && is_word_char(bytes[end]) {
        end += 1;
    }

    (start, end)
}

/// Converts a UTF-8 byte offset to an LSP [`Position`] (line, UTF-16 character).
///
/// If `offset` is beyond `text.len()`, it is clamped to the last valid position.
///
/// Used whenever a byte span from the compiler needs to be sent to the editor.
pub fn offset_to_position(text: &str, offset: usize) -> Position {
    let target = offset.min(text.len());
    let mut line = 0;
    let mut character = 0;
    let mut current_offset = 0;

    for c in text.chars() {
        if current_offset >= target {
            break;
        }
        if c == '\n' {
            line += 1;
            character = 0;
        } else {
            character += c.len_utf16() as u32;
        }
        current_offset += c.len_utf8();
    }

    Position { line, character }
}

/// Converts a run of non-decreasing byte offsets to LSP [`Position`]s in a single
/// pass over the text.
///
/// [`offset_to_position`] restarts from byte 0 on every call, so converting one
/// offset per token — what the semantic-tokens pass does — costs O(n²) in the
/// document length. Callers that visit offsets in non-decreasing order keep a
/// cursor instead and walk the document once.
///
/// Out-of-order or non-char-boundary offsets are still handled correctly: the
/// cursor falls back to a full scan rather than rejecting them.
pub struct PositionCursor<'a> {
    text: &'a str,
    offset: usize,
    line: u32,
    character: u32,
}

impl<'a> PositionCursor<'a> {
    pub fn new(text: &'a str) -> Self {
        PositionCursor {
            text,
            offset: 0,
            line: 0,
            character: 0,
        }
    }

    /// Advances the cursor to `offset` and returns the position there.
    pub fn position_at(&mut self, offset: usize) -> Position {
        let target = offset.min(self.text.len());
        if target < self.offset || !self.text.is_char_boundary(target) {
            return offset_to_position(self.text, target);
        }

        for c in self.text[self.offset..target].chars() {
            if c == '\n' {
                self.line += 1;
                self.character = 0;
            } else {
                self.character += c.len_utf16() as u32;
            }
        }
        self.offset = target;

        Position {
            line: self.line,
            character: self.character,
        }
    }
}

/// Returns the indices of non-redundant ranges from a slice, discarding those that
/// are strictly contained within (or identical to an earlier) range.
///
/// Used by [`references`](crate::references) and [`rename`](crate::rename) to
/// deduplicate symbol occurrences when the same identifier appears in multiple
/// overlapping spans in the `symbol_map`.
///
/// # Definition of redundancy
/// Range `r1` is redundant if there exists another range `r2` such that `r2` is
/// strictly contained within `r1` (or is identical and has a smaller index).
///
/// # Returns
/// A `Vec<usize>` of the indices from `ranges` that are **not** redundant, preserving
/// the original order.
///
/// # Complexity
/// `O(n log n)`.  Sorting by `(start asc, end desc)` puts every range that can be
/// contained in `ranges[i]` into the suffix after `i`, so a suffix minimum over the
/// end positions answers "does a contained range exist" in constant time per entry.
/// The pairwise scan this replaces was quadratic, and a rename touching a symbol used
/// a few hundred times ran it on every affected file.
pub fn deduplicate_range_indices(ranges: &[Range]) -> Vec<usize> {
    let key = |p: Position| (p.line, p.character);
    let mut order: Vec<usize> = (0..ranges.len()).collect();
    order.sort_unstable_by_key(|&i| {
        let r = ranges[i];
        // End descending, so equal-start ranges that are contained sort later.
        (key(r.start), std::cmp::Reverse(key(r.end)), i)
    });

    // `suffix_min[p]` is the smallest end position among `order[p..]`.
    let mut suffix_min: Vec<(u32, u32)> = vec![(u32::MAX, u32::MAX); order.len() + 1];
    for p in (0..order.len()).rev() {
        suffix_min[p] = suffix_min[p + 1].min(key(ranges[order[p]].end));
    }

    let mut result = Vec::new();
    let mut p = 0;
    while p < order.len() {
        // Identical ranges sort adjacently; only the first of the group survives.
        let group = ranges[order[p]];
        let mut group_end = p + 1;
        while group_end < order.len() && ranges[order[group_end]] == group {
            group_end += 1;
        }

        // Anything after the group starts at or after `group.start`, so a smaller
        // or equal end position means a strictly contained range exists.
        if suffix_min[group_end] > key(group.end) {
            result.push(order[p]);
        }
        p = group_end;
    }

    result.sort_unstable();
    result
}

/// Byte-offset → [`Position`] conversion backed by a precomputed line table.
///
/// [`offset_to_position`] rescans the document from byte 0 on every call, so a
/// caller converting many offsets over the same text — diagnostics, references,
/// rename edits — costs `O(offsets × document)`.  Building the table once makes
/// each conversion `O(log lines + line length)` and, unlike [`PositionCursor`],
/// imposes no ordering requirement on the offsets.
pub struct LineIndex<'a> {
    text: &'a str,
    /// Byte offset of the start of each line; always begins with `0`.
    line_starts: Vec<usize>,
}

impl<'a> LineIndex<'a> {
    pub fn new(text: &'a str) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            text.bytes()
                .enumerate()
                .filter(|&(_, b)| b == b'\n')
                .map(|(i, _)| i + 1),
        );
        LineIndex { text, line_starts }
    }

    /// Converts an absolute byte offset to an LSP position, clamping past-the-end
    /// offsets and rounding offsets inside a character up to its end — the same
    /// behaviour as [`offset_to_position`].
    pub fn position(&self, offset: usize) -> Position {
        let mut target = offset.min(self.text.len());
        while !self.text.is_char_boundary(target) {
            target += 1;
        }

        let line = self.line_starts.partition_point(|&start| start <= target) - 1;
        let character = self.text[self.line_starts[line]..target]
            .chars()
            .map(|c| c.len_utf16() as u32)
            .sum();

        Position {
            line: line as u32,
            character,
        }
    }
}

/// Groups the occurrence tuples produced by
/// [`DocumentState::find_matching_entities`](crate::state::DocumentState::find_matching_entities)
/// by URI and converts them into deduplicated LSP ranges.
///
/// Each tuple is `(uri, text, rel_start, rel_end, script_start)`, where the span
/// endpoints are **relative** to the region's `src_bytes`, so `script_start` shifts
/// them into the absolute file coordinates an LSP `Position` needs.
///
/// One [`LineIndex`] is built per file rather than rescanning the document for every
/// occurrence. Shared by [`references`](crate::references) and [`rename`](crate::rename),
/// which spelled the same grouping out separately.
pub fn occurrences_to_ranges(
    entities: Vec<crate::state::EntityOccurrence>,
) -> Vec<(String, Vec<Range>)> {
    /// A file's text and the absolute byte spans to convert against it.
    type FileSpans = (Arc<String>, Vec<(usize, usize)>);

    let mut by_uri: HashMap<String, FileSpans> = HashMap::new();
    for (state_uri, text, start, end, script_start) in entities {
        let abs_start = rel_to_abs_offset(start, script_start) as usize;
        let abs_end = rel_to_abs_offset(end, script_start) as usize;
        by_uri
            .entry(state_uri)
            .or_insert_with(|| (text, Vec::new()))
            .1
            .push((abs_start, abs_end));
    }

    by_uri
        .into_iter()
        .map(|(state_uri, (text, offsets))| {
            let lines = LineIndex::new(&text);
            let ranges: Vec<Range> = offsets
                .into_iter()
                .map(|(start, end)| Range {
                    start: lines.position(start),
                    end: lines.position(end),
                })
                .collect();
            let kept = deduplicate_range_indices(&ranges)
                .into_iter()
                .map(|i| ranges[i])
                .collect();
            (state_uri, kept)
        })
        .collect()
}

/// Converts a relative byte offset (within a region's `src_bytes`) to the
/// absolute byte offset in the whole file by adding `script_start`.
///
/// `script_start` is the absolute file byte position where the region's
/// `src_bytes` begins. When the region covers the whole file (no `@def`),
/// `script_start` is 0 and the relative and absolute offsets are equal.
///
/// # Parameters
/// * `rel_offset`  — The byte offset within the region's `src_bytes`.
/// * `script_start` — The absolute file byte offset of the region's start.
///
/// # Returns
/// The absolute byte offset in the whole file.
pub fn rel_to_abs_offset(rel_offset: u32, script_start: usize) -> u32 {
    rel_offset.saturating_add(script_start as u32)
}

/// Converts an absolute byte offset in the whole file to a relative byte
/// offset within the region's `src_bytes` by subtracting `script_start`.
///
/// The result is saturated to 0 when the input is below `script_start` so
/// callers do not need to bounds-check before calling.
///
/// # Parameters
/// * `abs_offset`   — The byte offset in the whole file.
/// * `script_start` — The absolute file byte offset of the region's start.
///
/// # Returns
/// The relative byte offset within the region's `src_bytes`.
pub fn abs_to_rel_offset(abs_offset: u32, script_start: usize) -> u32 {
    abs_offset.saturating_sub(script_start as u32)
}

/// Shifts a [`SourceSpan`] from relative coordinates (within a region's
/// `src_bytes`) to absolute file coordinates by adding `script_start` to both
/// endpoints. The `region_id` is preserved.
///
/// Used everywhere the LSP needs to surface a span that was produced by the
/// compiler/parser/lexer — those are always relative — as an LSP `Position`.
pub fn rel_to_abs_span(span: SourceSpan, script_start: usize) -> SourceSpan {
    SourceSpan::new(
        span.region_id,
        rel_to_abs_offset(span.start, script_start),
        rel_to_abs_offset(span.end, script_start),
    )
}

/// Shifts a [`SourceSpan`] from absolute file coordinates to relative
/// coordinates (within a region's `src_bytes`) by subtracting `script_start`
/// from both endpoints. The `region_id` is preserved.
///
/// Used when the LSP receives a cursor position and needs to look up an
/// entity or token by its byte offset; the byte offset the LSP derives from
/// an LSP `Position` is always absolute in the file.
pub fn abs_to_rel_span(span: SourceSpan, script_start: usize) -> SourceSpan {
    SourceSpan::new(
        span.region_id,
        abs_to_rel_offset(span.start, script_start),
        abs_to_rel_offset(span.end, script_start),
    )
}
