use std::path::Path;

use common::{
    core_error::{CoreError, ScriptError},
    intern::Intern,
    metadata::ChernSettings,
    reporter::diagnostic::Reporter,
    symbols::ModuleId,
};
use script_lib::{
    lexer::Lexer,
    modules::{self},
    parser::{self, ast::AstInfo},
    semantic::{
        constraint_resolver::ConstraintResolver, name_resolver::NamespaceResolver,
        type_resolver::TypeResolver,
    },
};

// Maybe this shouldn't take metadata externally
pub fn interpret_chern_cfg(path: &Path, settings: &ChernSettings) -> Result<(), CoreError> {
    let mut interner = Intern::init();

    // Doing this first since if modules were identified during the parsing stage any
    // syntax error within another module would not be reportable since the parser failed.
    let mut program = modules::extract_modules(path, settings, &mut interner)?;
    let mut reporter = Reporter::new();

    let mut asts: Vec<AstInfo> = Vec::new();

    // Need to separate namespace resolution and type resolver because if the modules namespaces
    // aren't resolved first, then type resolution isn't possible since it could be using types
    // from elsewhere, which are not known yet.
    for mod_idx in 0..program.mods.len() {
        let module = &program.mods[mod_idx];
        let toks = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
            .tokenize(&mut interner);

        let ast_info = match parser::parse(settings, &module, &toks, &mut interner) {
            Ok(info) => info,
            Err(script_err) => match script_err {
                ScriptError::Parser(mut diags) | ScriptError::Semantic(mut diags) => {
                    reporter.diags.append(&mut diags);
                    continue;
                }
                e => return Err(e.into()),
            },
        };

        match NamespaceResolver::new(settings, &ast_info, &interner, module.mod_id, &mut program)
            .resolve()
        {
            Ok(_) => (),
            Err(mut diags) => reporter.diags.append(&mut diags),
        }

        asts.push(ast_info);
    }

    if !reporter.diags.is_empty() {
        return Err(ScriptError::Semantic(reporter.diags).into());
    }

    // I don't know
    for i in 0..program.mods.len() {
        let mod_id = ModuleId::new(i);
        match TypeResolver::new(settings, &asts[i], mod_id, &interner, &mut program).resolve() {
            Ok(_) => (),
            Err(mut diags) => reporter.diags.append(&mut diags),
        };

        match ConstraintResolver::new(settings, &asts[i], &interner, mod_id, &mut program).resolve()
        {
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
