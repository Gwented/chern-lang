use std::path::Path;

use chrn_utils::{id_types::ModuleId, intern::Intern};
use common::{
    chrn_settings::ChrnSettings,
    core_error::{ConfigLoadError, CoreError, ScriptError},
    reporter::diagnostic::Reporter,
};
use script_lib::{
    modules::{self},
    parser::ast::AstInfo,
    script_compiler::ScriptCompiler,
    semantic::{
        constraint_resolver::{ConstraintResolver, value_context::ValueContext},
        name_resolver::NamespaceResolver,
        type_resolver::{TypeResolver, type_context::TypeContext},
    },
};

// Maybe this shouldn't take metadata externally
pub fn interpret_chrn_cfg(path: &Path, settings: &ChrnSettings) -> Result<(), CoreError> {
    let mut interner = Intern::init();

    // Doing this first since if modules were identified during the parsing stage any
    // syntax error within another module would not be reportable since the parser failed.
    let mut script_compiler = modules::extract_modules(path, settings, &mut interner)?;
    let mut reporter = Reporter::new();

    let mut asts: Vec<AstInfo> = Vec::new();

    // Need to separate namespace resolution and type resolver because if the modules namespaces
    // aren't resolved first, then type resolution isn't possible since it could be using types
    // from elsewhere, which are not known yet.
    for mod_idx in 0..script_compiler.mods.len() {
        let module = &script_compiler.mods[mod_idx];
        let toks =
            script_lib::lexer::Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
                .tokenize(&mut interner);

        // Maybe it should just return diagnostics normally and let the caller return whatever
        // script error it wants
        let ast_info = match script_lib::parser::parse(settings, &module, &toks, &mut interner) {
            Ok(info) => info,
            Err((unfinished_ast, mut diags)) => {
                reporter.diags.append(&mut diags);
                unfinished_ast
            }
        };

        NamespaceResolver::new(
            settings,
            &ast_info,
            &interner,
            module.mod_id,
            &mut script_compiler,
        )
        .resolve()
        .unwrap_or_else(|mut diags| reporter.diags.append(&mut diags));

        asts.push(ast_info);
    }

    if !reporter.diags.is_empty() {
        return Err(ScriptError::Semantic(reporter.diags).into());
    }

    let mut ty_ctx = TypeContext::new();
    for i in 0..script_compiler.mods.len() {
        let mod_id = ModuleId::new(i);
        TypeResolver::new(
            settings,
            &asts[i],
            mod_id,
            &mut ty_ctx,
            &interner,
            &mut script_compiler,
        )
        .resolve()
        .unwrap_or_else(|mut diags| reporter.diags.append(&mut diags));
    }

    if !reporter.diags.is_empty() {
        return Err(ScriptError::Semantic(reporter.diags).into());
    }

    todo!("Ignoring constraints for now");

    // For ensuring a stateful piece of context is retained for resolving all module variables.
    // This is not a value resolver
    let mut val_ctx = ValueContext::new();

    for i in 0..script_compiler.mods.len() {
        let mod_id = ModuleId::new(i);
        ConstraintResolver::new(
            settings,
            &asts,
            &interner,
            mod_id,
            &mut val_ctx,
            &mut script_compiler,
        )
        .resolve()
        .unwrap_or_else(|mut diags| reporter.diags.append(&mut diags));
    }

    if !reporter.diags.is_empty() {
        return Err(ScriptError::Semantic(reporter.diags).into());
    }

    todo!("Out of constraitns");

    Ok(())
}
