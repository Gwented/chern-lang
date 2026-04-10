use std::path::Path;

use common::{core_error::CoreError, intern::Intern};
use script_lib::{
    lexer::Lexer,
    modules::{self, Program},
    parser::{self, ast::AstInfo},
    semantic::{constraint_resolver::ConstraintResolver, type_resolver::TypeResolver},
};

// Maybe this shouldn't take metadata externally
pub fn interpret_chern_cfg(path: &Path) -> Result<(), CoreError> {
    let mut interner = Intern::init();

    // Deciding on doing this first since if modules were identified during the parsing stage any
    // syntax error within another module would not be reportable since the parser failed.

    // Not entirely sure what to do with program yet so staying outside for now
    let mut program = Program::new(None);

    let modules = modules::extract_modules(path, &mut interner)?;

    //Temp
    program.mods = modules;

    let mut all_asts: Vec<AstInfo> = Vec::new();

    // Storing namespaces so that referencing can be made to existent outer module variables
    for module in &mut program.mods {
        let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&module.metadata, &toks, &mut interner)?;
        TypeResolver::new(&ast_info, &interner, module).resolve();

        all_asts.push(ast_info);
    }

    // dbg!(program.mods);
    // panic!("Namespaces");
    for (i, module) in program.mods.iter_mut().enumerate() {
        ConstraintResolver::new(&all_asts[i], &module.metadata, &interner, &mut module.table)
            .resolve();
    }

    // Maybe bind is now gotten from module resolution

    // IR will be very different
    Ok(())
}
