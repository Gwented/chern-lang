use chrn_utils::intern::Intern;
use compilation::script_compiler::ScriptCompiler;

use crate::dump_settings::DumpSettings;

pub struct PrintContext<'a> {
    compiler: &'a ScriptCompiler,
    indent: usize,
    interner: &'a Intern,
}

impl PrintContext<'_> {
    pub(crate) fn new<'a>(
        compiler: &'a ScriptCompiler,
        indent: usize,
        interner: &'a Intern,
    ) -> PrintContext<'a> {
        PrintContext {
            compiler,
            indent,
            interner,
        }
    }

    pub(crate) fn increase_indent(&mut self) {
        self.indent += 4;
    }

    pub(crate) fn decrease_indent(&mut self) {
        self.indent -= 4;
    }
}

// TEST:
// More like stringifying

pub fn print_env(compiler: &ScriptCompiler, settings: &DumpSettings, interner: &Intern) -> String {
    let mut ctx = PrintContext::new(compiler, 0, interner);
    todo!()
}
