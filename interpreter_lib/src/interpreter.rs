use std::{fs, path::Path};

use common::{
    config_loader::ChernConfigLoader,
    core_error::{ConfigLoadError, CoreError, ScriptError},
    intern::Intern,
};
use script_lib::{
    lexer::Lexer,
    modules::{self, Program},
    parser::{self, ast::AstInfo},
    semantic::{
        constraint_resolver::ConstraintResolver, representation::Table, type_resolver::TypeResolver,
    },
};

// Maybe this shouldn't take metadata externally
pub fn interpret_chern_cfg(path: &Path) -> Result<(), CoreError> {
    let mut interner = Intern::init();

    // let time_stamp = std::time::UNIX_EPOCH.elapsed().unwrap().as_secs() / 10000;

    // Deciding on doing this first since if modules were identified during the parsing stage any
    // syntax error within another module would not be reportable since the parser failed.

    // Not entirely sure what to do with program yet so staying outside for now
    let mut program = Program::new(None);

    let modules = match modules::extract_modules(path, &mut interner) {
        Ok(mods) => mods,
        Err(script_err) => match script_err {
            ConfigLoadError::Unclosed(_) => todo!("Internal error"),
            ConfigLoadError::IO(error) => panic!("{}", error),
        },
    };

    //Temp
    program.mods = modules;

    for module in &mut program.mods {
        let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
            .tokenize(&mut interner);
        let ast_info = parser::parse(&module.metadata, &toks, &mut interner)?;

        // This needs to be sorted
        TypeResolver::new(&ast_info, &module.metadata, &interner, &mut module.table).resolve();
        ConstraintResolver::new(&ast_info, &module.metadata, &interner, &mut module.table)
            .resolve();
    }

    // Maybe bind is now gotten from module resolution

    // IR will be very different
    Ok(())
}
