pub const RED: &str = "\x1b[31m";
pub const GREEN: &str = "\x1b[32m";
pub const ORANGE: &str = "\x1b[33m";
pub const CYAN: &str = "\x1b[38m";
pub const NC: &str = "\x1b[0m";

pub const UNDERLINE: &str = "\x1b[21m";
pub const BOLD: &str = "\x1b[1m";
pub const BOLD_CYAN: &str = "\x1b[1m\x1b[38m";

/// Returns green ANSI code and NC
pub fn get_green(can_color: bool) -> (&'static str, &'static str) {
    if can_color { (GREEN, NC) } else { ("", "") }
}

/// Returns orange ANSI code and NC
pub fn get_orange(can_color: bool) -> (&'static str, &'static str) {
    if can_color { (ORANGE, NC) } else { ("", "") }
}

/// Returns Red ANSI code and NC
pub fn get_red(can_color: bool) -> (&'static str, &'static str) {
    if can_color { (RED, NC) } else { ("", "") }
}

/// Returns Cyan ANSI code and NC
pub fn get_cyan(can_color: bool) -> (&'static str, &'static str) {
    if can_color { (CYAN, NC) } else { ("", "") }
}

/// Returns orange ANSI code and NC
pub fn get_bold_cyan(can_color: bool) -> (&'static str, &'static str) {
    if can_color { (BOLD_CYAN, NC) } else { ("", "") }
}

/// Returns Bold ANSI code and NC
pub fn get_bold(can_color: bool) -> (&'static str, &'static str) {
    if can_color { (BOLD, NC) } else { ("", "") }
}
