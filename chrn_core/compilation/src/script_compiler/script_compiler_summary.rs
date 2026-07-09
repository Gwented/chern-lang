#[derive(Debug, Default)]
pub struct ScriptCompilerSummary {
    /// If reached, this will be set to the amount that needed to be reached to be considered exceeded.
    /// This contains the max module count for the given session.
    pub exceeded_max_mods: Option<u16>,
}

impl ScriptCompilerSummary {
    pub const fn new() -> ScriptCompilerSummary {
        ScriptCompilerSummary {
            exceeded_max_mods: None,
        }
    }
}
