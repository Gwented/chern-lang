//TEST: No longer has use but is useful to keep in case of any future use
// ScriptCompilerSettings :)
/// Settings given before running a chrn language instance, which allows for external tooling
/// comptabilities, such as cli args.
#[derive(Debug)]
pub struct ChrnSettings {}

impl ChrnSettings {
    pub fn new() -> ChrnSettings {
        ChrnSettings {}
    }
}

impl Default for ChrnSettings {
    fn default() -> Self {
        Self {}
    }
}
