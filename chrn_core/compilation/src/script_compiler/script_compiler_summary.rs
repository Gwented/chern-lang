#[derive(Debug)]
pub struct ScriptCompilerSummary {
    // suppresed_diags: u8,
    pub exceeded_max_mods: bool,
}

impl ScriptCompilerSummary {
    pub fn new() -> ScriptCompilerSummary {
        ScriptCompilerSummary {
            exceeded_max_mods: false,
        }
    }
}
