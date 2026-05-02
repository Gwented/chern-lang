use std::io::IsTerminal;

pub struct CliConfig {
    pub can_color: bool,
}

impl CliConfig {
    pub fn new() -> CliConfig {
        let can_color = if std::io::stdout().is_terminal() && std::io::stderr().is_terminal() {
            true
        } else {
            false
        };

        CliConfig { can_color }
    }
}
