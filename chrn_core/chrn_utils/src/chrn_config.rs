//TEST: No longer has use but is useful to keep in case of any future use
// ScriptCompilerSettings :)
/// Settings given before running a chrn language instance, which allows for external tooling
/// comptabilities, such as cli args.
#[derive(Debug)]
pub struct ChrnConfig {}

impl ChrnConfig {
    pub fn new() -> ChrnConfig {
        ChrnConfig {}
    }
}

impl Default for ChrnConfig {
    fn default() -> Self {
        Self {}
    }
}
