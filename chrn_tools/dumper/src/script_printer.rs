use compilation::{parser::ast::ast_concepts::AstInfo, script_compiler::ScriptCompiler};

pub struct ScriptPrinter<'a> {
    ast_info: &'a AstInfo,
    script_compiler: &'a ScriptCompiler,
    indent: u32,
}

impl ScriptPrinter<'_> {
    pub fn new<'a>(
        ast_info: &'a AstInfo,
        script_compiler: &'a ScriptCompiler,
    ) -> ScriptPrinter<'a> {
        ScriptPrinter {
            ast_info,
            script_compiler,
            indent: 0,
        }
    }

    // Needs to exist for correctness assurance reasons mostly
    pub fn fmt_details(&mut self) -> String {
        let mut details = String::new();
        for module in &self.script_compiler.mods.items {
            todo!("Hi modules")
        }
        panic!();
    }

    pub fn increase_indent(&mut self) {
        self.indent += 4;
    }

    pub fn decrease_indent(&mut self) {
        self.indent -= 4;
    }
}
