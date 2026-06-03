// colored crate also accounts for this but trying out this type of implementation which doesn't
// involve dependencies for future purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TerminalColorType {
    #[default]
    Ansi4,
    Ansi8,
    TrueColor,
}

impl TerminalColorType {
    pub fn detect() -> Self {
        // COLORTERM is set by many modern terminals (Windows Terminal, Kitty,
        // iTerm2, GNOME Terminal, etc.)
        if let Ok(val) = std::env::var("COLORTERM") {
            let lower = val.to_lowercase();
            if lower == "truecolor" || lower == "24bit" {
                return TerminalColorType::TrueColor;
            }
        }

        // TERM_PROGRAM is set by VS Code, iTerm2, Ghostty, etc. on all platforms.
        if let Ok(val) = std::env::var("TERM_PROGRAM") {
            if val == "vscode" || val == "iterm2" || val == "ghostty" {
                return TerminalColorType::TrueColor;
            }
        }

        // Windows Terminal sets WT_SESSION; ConEmu/CMDER sets ConEmuANSI.
        #[cfg(windows)]
        {
            if std::env::var("WT_SESSION").is_ok() || std::env::var("ConEmuANSI").is_ok() {
                return TerminalColorType::TrueColor;
            }
        }

        // TERM is the classic Unix variable (xterm-256color, screen-256color, etc.)
        if let Ok(val) = std::env::var("TERM") {
            if val.contains("256color") || val.contains("256") {
                return TerminalColorType::Ansi8;
            }
        }

        TerminalColorType::Ansi4
    }
}

// 4-bit ANSI reference constants (16-color palette).
pub const RED: &str = "\x1b[31m";
pub const GREEN: &str = "\x1b[32m";
pub const ORANGE: &str = "\x1b[33m";
pub const CYAN: &str = "\x1b[36m";
pub const GREY: &str = "\x1b[90m";
pub const NC: &str = "\x1b[0m";

pub const UNDERLINE: &str = "\x1b[21m";
pub const BOLD: &str = "\x1b[1m";
pub const BOLD_CYAN: &str = "\x1b[1m\x1b[36m";

/// Returns NC ANSI code.
pub fn get_nc(can_color: bool) -> &'static str {
    if can_color { NC } else { "" }
}

fn color_code(
    terminal: TerminalColorType,
    ansi4: &str,
    ansi8: &str,
    truecolor_rgb: [u8; 3],
) -> String {
    match terminal {
        TerminalColorType::Ansi4 => ansi4.to_string(),
        TerminalColorType::Ansi8 => ansi8.to_string(),
        TerminalColorType::TrueColor => {
            format!(
                "\x1b[38;2;{};{};{}m",
                truecolor_rgb[0], truecolor_rgb[1], truecolor_rgb[2]
            )
        }
    }
}

fn pair(can_color: bool, color: String) -> (String, &'static str) {
    if can_color {
        (color, NC)
    } else {
        (String::new(), "")
    }
}

/// Returns green ANSI code and NC.
pub fn get_green(can_color: bool, terminal: TerminalColorType) -> (String, &'static str) {
    pair(
        can_color,
        color_code(terminal, "\x1b[32m", "\x1b[38;5;34m", [0, 180, 0]),
    )
}

/// Returns grey ANSI code and NC.
pub fn get_grey(can_color: bool, terminal: TerminalColorType) -> (String, &'static str) {
    pair(
        can_color,
        color_code(terminal, "\x1b[90m", "\x1b[38;5;244m", [169, 169, 169]),
    )
}

/// Returns orange ANSI code and NC.
pub fn get_orange(can_color: bool, terminal: TerminalColorType) -> (String, &'static str) {
    pair(
        can_color,
        color_code(terminal, "\x1b[33m", "\x1b[38;5;214m", [255, 165, 0]),
    )
}

/// Returns a lighter red ANSI code and NC.
pub fn get_red(can_color: bool, terminal: TerminalColorType) -> (String, &'static str) {
    pair(
        can_color,
        color_code(terminal, "\x1b[91m", "\x1b[38;5;210m", [255, 100, 100]),
    )
}

/// Returns a lighter ocean-blue ANSI code and NC.
pub fn get_cyan(can_color: bool, terminal: TerminalColorType) -> (String, &'static str) {
    pair(
        can_color,
        color_code(terminal, "\x1b[96m", "\x1b[38;5;81m", [64, 164, 223]),
    )
}

/// Returns bold ocean-blue ANSI code and NC.
pub fn get_bold_cyan(can_color: bool, terminal: TerminalColorType) -> (String, &'static str) {
    if can_color {
        let bold = "\x1b[1m";
        let color = color_code(terminal, "\x1b[96m", "\x1b[38;5;81m", [64, 164, 223]);
        (format!("{bold}{color}"), NC)
    } else {
        (String::new(), "")
    }
}

/// Returns Bold ANSI code and NC. Bold is universal across terminal types.
pub fn get_bold(can_color: bool) -> (String, &'static str) {
    pair(can_color, BOLD.to_string())
}
