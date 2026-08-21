use std::io::{self, IsTerminal};

use colorc::color_type::TerminalColorType;

// This more so exists because it may need to exist as something more, but it currently just stores
// information that other settings already use so it's mostly here just in case
/// Settings specific to the renderer
//TODO: Will need to decide where this lands
#[derive(Debug, Clone)]
pub(crate) struct TerminalRenderConfig {
    // A bit redundant with CLI config but I don't know about forcing passing around the same two
    // structures just to avoid one bit of duplication here.
    pub(crate) can_color: bool,
    pub(crate) terminal_type: TerminalColorType,
}

impl TerminalRenderConfig {
    pub(crate) fn new(can_color: bool, terminal_type: TerminalColorType) -> TerminalRenderConfig {
        TerminalRenderConfig {
            can_color,
            terminal_type,
        }
    }

    pub(crate) fn init() -> TerminalRenderConfig {
        // Simple check of if it's a terminal or not through io to catch basic cases
        let can_color = io::stdin().is_terminal() || io::stderr().is_terminal();
        let terminal_type = TerminalColorType::detect();
        TerminalRenderConfig {
            can_color,
            terminal_type,
        }
    }
}
