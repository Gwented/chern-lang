use std::path::Path;

use common::{
    core_error::{CoreError, ScriptError},
    intern::Intern,
    reporter::diagnostic::Reporter,
};
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
    let mut program = modules::extract_modules(path, &mut interner)?;

    let mut reporter = Reporter::new();

    let mut all_asts: Vec<AstInfo> = Vec::new();

    // Storing namespaces first so that referencing can be made to accessible to other modules
    for module in &mut program.mods {
        let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
            .tokenize(&mut interner);

        let ast_info = match parser::parse(&module.metadata, &toks, &mut interner) {
            Ok(info) => info,
            Err(script_err) => match script_err {
                ScriptError::Parser(mut diagnostics) | ScriptError::Semantic(mut diagnostics) => {
                    reporter.diags.append(&mut diagnostics);
                    continue;
                }
                e => return Err(e.into()),
            },
        };

        match TypeResolver::new(&ast_info, &interner, module).resolve() {
            Ok(_) => (),
            Err(mut diags) => reporter.diags.append(&mut diags),
        };

        all_asts.push(ast_info);
    }

    if !reporter.diags.is_empty() {
        // Suspicious into usage
        return Err(ScriptError::Semantic(reporter.diags).into());
    }

    // I don't know
    for i in 0..program.mods.len() {
        match ConstraintResolver::new(&all_asts[i], &interner, i, &mut program).resolve() {
            Ok(_) => (),
            Err(mut diags) => reporter.diags.append(&mut diags),
        };
    }

    if !reporter.diags.is_empty() {
        // Suspicious into usage
        return Err(ScriptError::Semantic(reporter.diags).into());
    }

    // Maybe bind is now gotten from module resolution

    // IR will be very different
    Ok(())
}
