use std::fmt::Write;

/// Returns `true` when `s` cannot be safely emitted as a YAML plain scalar
/// and must be wrapped in double quotes (or otherwise escaped).
///
/// This is a conservative check: anything ambiguous to a YAML 1.2 loader
/// gets quoted so round-tripping through the JSON test fixtures is stable.
pub(super) fn needs_quoting(s: &str) -> bool {
    if s.is_empty() {
        return true;
    }

    // Multi-line strings: we always quote these in this renderer (no `|`
    // block scalars) so the output mirrors the JSON escaped form.
    if s.contains('\n') || s.contains('\r') {
        return true;
    }

    // Tab characters are not safe in plain scalars (they indicate alignment
    // in some loaders); also any control character.
    if s.chars().any(|c| (c as u32) < 0x20) {
        return true;
    }

    // Leading or trailing whitespace is not allowed in plain scalars.
    if s != s.trim() {
        return true;
    }

    // Disallowed leading characters in plain flow / block scalars.
    let first = s.chars().next().unwrap();
    if matches!(
        first,
        '-' | '?'
            | ','
            | '['
            | ']'
            | '{'
            | '}'
            | '#'
            | '&'
            | '*'
            | '!'
            | '|'
            | '>'
            | '\''
            | '"'
            | '%'
            | '@'
            | '`'
    ) {
        return true;
    }

    // A leading `:` is reserved (could be a key in a flow context).
    if s.starts_with(':') {
        return true;
    }

    // `:` followed by space is the explicit key/value separator in plain
    // scalars; `:#` would be ambiguous too.
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == ':' && matches!(chars.peek(), Some(' ') | Some('\t') | Some('#') | None) {
            return true;
        }
        if c == ' ' && matches!(chars.peek(), Some('#')) {
            return true;
        }
    }

    // Reserved words that a YAML loader would parse as a typed scalar.
    if matches!(
        s,
        "true"
            | "false"
            | "True"
            | "False"
            | "TRUE"
            | "FALSE"
            | "yes"
            | "no"
            | "Yes"
            | "No"
            | "YES"
            | "NO"
            | "on"
            | "off"
            | "On"
            | "Off"
            | "ON"
            | "OFF"
            | "null"
            | "Null"
            | "NULL"
            | "~"
    ) {
        return true;
    }

    // Anything that parses as a number should be quoted, otherwise
    // `region_path: 42` is ambiguous with the integer 42.
    if looks_like_number(s) {
        return true;
    }

    false
}

/// Returns `true` if `s` parses as a YAML 1.2 integer or float literal
/// when interpreted as a plain scalar.
fn looks_like_number(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }

    let bytes = s.as_bytes();
    let mut i = 0;

    if matches!(bytes[0], b'+' | b'-') {
        i += 1;
        if i == bytes.len() {
            return false;
        }
    }

    // Hex / octal / binary: 0x..., 0o..., 0b...
    if i + 1 < bytes.len() && bytes[i] == b'0' {
        match bytes[i + 1] {
            b'x' | b'X' => {
                return bytes[i + 2..].iter().all(|b| b.is_ascii_hexdigit());
            }
            b'o' | b'O' => return bytes[i + 2..].iter().all(|b| matches!(b, b'0'..=b'7')),
            b'b' | b'B' => return bytes[i + 2..].iter().all(|b| matches!(b, b'0' | b'1')),
            _ => {}
        }
    }

    let mut saw_digit = false;
    let mut saw_dot = false;
    let mut saw_e = false;
    let mut after_e = i;

    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b'0'..=b'9' => {
                saw_digit = true;
                i += 1;
            }
            b'.' if !saw_dot && !saw_e => {
                saw_dot = true;
                i += 1;
            }
            b'e' | b'E' if !saw_e && saw_digit => {
                saw_e = true;
                saw_digit = false;
                i += 1;
                after_e = i;
                if i < bytes.len() && matches!(bytes[i], b'+' | b'-') {
                    i += 1;
                }
            }
            _ => return false,
        }
    }

    if saw_e {
        // After `e[+-]?` we need at least one digit.
        i > after_e
    } else {
        saw_digit
    }
}

/// Appends `s` to `out` as a YAML scalar, choosing plain or double-quoted
/// form based on [`needs_quoting`].
pub(super) fn push_yaml_str(out: &mut String, s: &str) {
    if !needs_quoting(s) {
        out.push_str(s);
        return;
    }

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
    use super::{needs_quoting, push_yaml_str};

    fn emit(s: &str) -> String {
        let mut buf = String::new();
        push_yaml_str(&mut buf, s);
        buf
    }

    #[test]
    fn plain_ascii_is_emitted_bare() {
        assert!(!needs_quoting("hello"));
        assert_eq!(emit("hello"), "hello");
    }

    #[test]
    fn path_like_strings_are_plain() {
        assert!(!needs_quoting("/tmp/a.chrn"));
        assert!(!needs_quoting("foo/bar/baz.txt"));
        assert_eq!(emit("/tmp/a.chrn"), "/tmp/a.chrn");
    }

    #[test]
    fn empty_string_is_quoted() {
        assert!(needs_quoting(""));
        assert_eq!(emit(""), "\"\"");
    }

    #[test]
    fn string_containing_colon_space_is_quoted() {
        assert!(needs_quoting("a: b"));
        assert_eq!(emit("a: b"), "\"a: b\"");
    }

    #[test]
    fn string_starting_with_dash_is_quoted() {
        assert!(needs_quoting("-foo"));
        assert_eq!(emit("-foo"), "\"-foo\"");
    }

    #[test]
    fn reserved_words_are_quoted() {
        for w in [
            "true", "false", "null", "~", "yes", "no", "on", "off", "True", "FALSE", "NULL",
        ] {
            assert!(needs_quoting(w), "expected {w} to need quoting");
        }
    }

    #[test]
    fn numbers_look_like_numbers_are_quoted() {
        for n in ["0", "42", "-7", "+3.14", "1e10", "0x1f", "0b1010", "0o17"] {
            assert!(needs_quoting(n), "expected {n} to need quoting");
        }
    }

    #[test]
    fn strings_with_leading_or_trailing_whitespace_are_quoted() {
        assert!(needs_quoting(" leading"));
        assert!(needs_quoting("trailing "));
    }

    #[test]
    fn multiline_strings_are_quoted() {
        assert!(needs_quoting("a\nb"));
        assert_eq!(emit("a\nb"), "\"a\\nb\"");
    }

    #[test]
    fn short_form_escapes_match_json() {
        assert_eq!(emit("\x08\x0c\n\r\t"), "\"\\b\\f\\n\\r\\t\"");
    }

    #[test]
    fn other_control_chars_use_unicode_escape() {
        assert_eq!(emit("\x01\x1f"), "\"\\u0001\\u001f\"");
    }

    #[test]
    fn quote_and_backslash_are_escaped_when_quoting() {
        // The leading `-` forces quoting; inside the quoted form, the
        // embedded `"` and `\` get the JSON-compatible escaping.
        assert_eq!(emit("-\"\\c"), "\"-\\\"\\\\c\"");
    }

    #[test]
    fn unicode_passes_through_unquoted() {
        assert!(!needs_quoting("héllo"));
        assert_eq!(emit("héllo"), "héllo");
    }
}
