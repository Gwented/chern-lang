//TEST: No longer has use but is useful to keep in case of any future use
/// Config given before running a chrn language instance, which allows for external tooling
/// capabilities, such as cli args.
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
