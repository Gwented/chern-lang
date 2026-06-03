use std::io::{self, IsTerminal};

use common::color::TerminalColorType;

//TODO: Will have more eventually. Maybe.
#[derive(Debug)]
pub(crate) struct RenderSettings {
    // A bit redundant with CLI config but I don't know about forcing passing around the same two
    // structures just to avoid one bit of duplication here.
    pub(crate) can_color: bool,
    pub(crate) terminal_type: TerminalColorType,
}

impl RenderSettings {
    pub(crate) fn new(can_color: bool, terminal_type: TerminalColorType) -> RenderSettings {
        RenderSettings {
            can_color,
            terminal_type,
        }
    }

    pub(crate) fn init() -> RenderSettings {
        // Simple check of if it's a terminal or not through io to catch basic cases
        let can_color = io::stdin().is_terminal() || io::stderr().is_terminal();
        let terminal_type = TerminalColorType::detect();
        RenderSettings {
            can_color,
            terminal_type,
        }
    }
}
