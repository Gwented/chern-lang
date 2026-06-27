use common::color::TerminalColorType;

pub struct CliConfig {
    pub terminal_color_type: TerminalColorType,
}

impl CliConfig {
    pub fn new() -> CliConfig {
        let terminal_type = TerminalColorType::detect();

        CliConfig {
            terminal_color_type: terminal_type,
        }
    }
}
