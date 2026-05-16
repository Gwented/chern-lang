use tower_lsp::lsp_types::Position;
use tower_lsp::lsp_types::Range;
use tower_lsp::lsp_types::TextDocumentContentChangeEvent;

pub fn extract_word_at(line: &str, idx: usize) -> String {
    // idx is character index; clamp
    let idx = idx.min(line.len());
    let bytes = line.as_bytes();
    // find start
    let mut start = idx;
    while start > 0 {
        let c = bytes[start - 1] as char;
        if c.is_alphanumeric()
            || c == '_'
            || c == '#'
            || c == '@'
            || c == '<'
            || c == '>'
            || c == ':'
            || c == '-'
        {
            start -= 1;
        } else {
            break;
        }
    }
    // find end
    let mut end = idx;
    while end < bytes.len() {
        let c = bytes[end] as char;
        if c.is_alphanumeric()
            || c == '_'
            || c == '#'
            || c == '@'
            || c == '<'
            || c == '>'
            || c == ':'
            || c == '-'
        {
            end += 1;
        } else {
            break;
        }
    }

    line[start..end].to_string()
}

pub fn apply_text_change(
    existing: &str,
    change: &TextDocumentContentChangeEvent,
) -> Result<String, String> {
    // If no range is provided, the client replaced the entire document
    if change.range.is_none() {
        return Ok(change.text.clone());
    }

    // Extract byte offsets for the range
    let range = match change.range {
        Some(r) => r,
        None => return Ok(change.text.clone()),
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

pub fn position_to_offset(text: &str, pos: Position) -> usize {
    let mut offset = 0;
    let mut line = 0;
    for ln in text.split_inclusive('\n') {
        if line == pos.line {
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
        line += 1;
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
    // move start backward while previous char is word-like
    while start > 0 {
        let c = bytes[start - 1] as char;
        if c.is_alphanumeric()
            || c == '_'
            || c == '#'
            || c == '@'
            || c == '<'
            || c == '>'
            || c == ':'
            || c == '-'
        {
            start -= 1;
        } else {
            break;
        }
    }

    let mut end = offset.min(len);
    while end < len {
        let c = bytes[end] as char;
        if c.is_alphanumeric()
            || c == '_'
            || c == '#'
            || c == '@'
            || c == '<'
            || c == '>'
            || c == ':'
            || c == '-'
        {
            end += 1;
        } else {
            break;
        }
    }

    (start, end)
}

/// Convert a byte offset into an LSP Position (line, character in UTF-16 code units).
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

/// Returns which indices in `ranges` are NOT redundant (i.e. are the most specific non-overlapping ranges).
/// A range is redundant if another range exists that is strictly contained within it.
/// For identical ranges, the earliest index wins.
pub fn deduplicate_range_indices(ranges: &[Range]) -> Vec<usize> {
    let mut result = Vec::new();
    for i in 0..ranges.len() {
        let r1 = &ranges[i];
        let mut is_redundant = false;
        for j in 0..ranges.len() {
            if i == j {
                continue;
            }
            let r2 = &ranges[j];

            let starts_after_or_at = r2.start.line > r1.start.line
                || (r2.start.line == r1.start.line && r2.start.character >= r1.start.character);
            let ends_before_or_at = r2.end.line < r1.end.line
                || (r2.end.line == r1.end.line && r2.end.character <= r1.end.character);

            if starts_after_or_at && ends_before_or_at {
                if r1.start != r2.start || r1.end != r2.end {
                    is_redundant = true;
                    break;
                } else if j < i {
                    is_redundant = true;
                    break;
                }
            }
        }
        if !is_redundant {
            result.push(i);
        }
    }
    result
}
