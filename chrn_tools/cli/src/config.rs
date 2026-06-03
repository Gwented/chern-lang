use std::io::IsTerminal;

use common::color::TerminalColorType;

pub struct CliConfig {
    pub can_color: bool,
    pub terminal_color_type: TerminalColorType,
}

impl CliConfig {
    pub fn new() -> CliConfig {
        let can_color = std::io::stdout().is_terminal() && std::io::stderr().is_terminal();
        let terminal_type = TerminalColorType::detect();

        CliConfig {
            can_color,
            terminal_color_type: terminal_type,
        }
    }
}
