/// Summary containing notable compiler notes during compilation
#[derive(Debug, Default)]
pub struct ScriptCompilerSummary {
    /// Detected during module graph stage.
    /// If reached, this will be set to the amount that needed to be reached to be considered exceeded.
    /// This contains the max module count for the given session so that it can stay dynamic.
    pub exceeded_max_mods: Option<u16>,
    // Maybe summary internally? For stage specific summary outputs having all be free fields would
    // be (Free fields is not a term) wait what
    // pub recursive_descent_exceeded: u16
}

impl ScriptCompilerSummary {
    pub const fn new() -> ScriptCompilerSummary {
        ScriptCompilerSummary {
            exceeded_max_mods: None,
        }
    }
}

pub trait CompilerSummary {}
