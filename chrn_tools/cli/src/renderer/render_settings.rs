use std::io::{self, IsTerminal};

//TODO: Will have more eventually. Maybe.
#[derive(Debug, Default)]
pub(crate) struct RenderSettings {
    pub(crate) can_color: bool,
}

impl RenderSettings {
    pub(crate) fn new(can_color: bool) -> RenderSettings {
        RenderSettings { can_color }
    }

    pub(crate) fn init() -> RenderSettings {
        // Simple check of if it's a terminal or not through io to catch basic cases
        let can_color = io::stdin().is_terminal() || io::stderr().is_terminal();
        RenderSettings { can_color }
    }
}
