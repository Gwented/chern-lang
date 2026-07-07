use std::fmt::Write;

/// Appends `s` to `out` as a JSON string literal (including surrounding quotes),
/// applying the required escape sequences.
///
/// Handles the short-form escapes `\"`, `\\`, `\b`, `\f`, `\n`, `\r`, `\t`, and
/// `\u00XX` for any other control character (`< 0x20`). Valid UTF-8 is passed
/// through unchanged.
pub(super) fn push_json_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\x08' => out.push_str("\\b"),
            '\x0c' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::push_json_str;

    fn escape(s: &str) -> String {
        let mut buf = String::new();
        push_json_str(&mut buf, s);
        buf
    }

    #[test]
    fn plain_ascii_is_quoted_only() {
        assert_eq!(escape("hello"), "\"hello\"");
    }

    #[test]
    fn empty_string() {
        assert_eq!(escape(""), "\"\"");
    }

    #[test]
    fn quote_and_backslash_escape() {
        assert_eq!(escape("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }

    #[test]
    fn short_form_escapes() {
        assert_eq!(escape("\x08\x0c\n\r\t"), "\"\\b\\f\\n\\r\\t\"");
    }

    #[test]
    fn other_control_chars_use_unicode_escape() {
        assert_eq!(escape("\x01\x1f"), "\"\\u0001\\u001f\"");
    }

    #[test]
    fn unicode_passes_through() {
        assert_eq!(escape("héllo"), "\"héllo\"");
    }
}
