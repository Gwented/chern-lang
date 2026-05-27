use std::path::Path;

use chrn_utils::{id_types::ModuleId, intern::Intern};
use common::{
    chrn_settings::ChrnSettings,
    core_error::{CoreError, ScriptError},
    reporter::diagnostic::Reporter,
};
use script_lib::{
    modules::{self},
    parser::ast::AstInfo,
    semantic::{
        constraint_resolver::ConstraintResolver,
        name_resolver::NamespaceResolver,
        type_resolver::{TypeResolver, type_context::TypeContext},
    },
};

// Maybe this shouldn't take metadata externally
pub fn interpret_chrn_cfg(path: &Path, settings: &ChrnSettings) -> Result<(), CoreError> {
    let mut interner = Intern::init();
    // let mut span_arena: Vec<Span> = Vec::new();

    // Doing this first since if modules were identified during the parsing stage any
    // syntax error within another module would not be reportable since the parser failed.
    let mut script_compiler = modules::extract_modules(path, settings, &mut interner)?;
    let mut reporter = Reporter::new();

    //TODO: May have to just make this into an Option<AstInfo>
    let mut asts: Vec<AstInfo> = Vec::new();

    // Need to separate namespace resolution and type resolver because if the modules namespaces
    // aren't resolved first, then type resolution isn't possible since it could be using types
    // from elsewhere, which are not known yet.
    for mod_idx in 0..script_compiler.mods.len() {
        let module = &script_compiler.mods[mod_idx];
        let metadata = match &module.src_metadata {
            Some(m) => m,
            None => continue,
        };

        let (toks, _) = script_lib::lexer::Lexer::new(&metadata.src_bytes, metadata.script_start)
            .tokenize(&mut interner);

        let ast_info = match script_lib::parser::parse(settings, &metadata, &toks, &mut interner) {
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

    //FIX: AstId position should be a direct tie, not sequential
    let mut ty_ctx = TypeContext::new();
    for i in 0..script_compiler.mods.len() {
        let module = &script_compiler.mods[i];
        if module.src_metadata.is_none() {
            continue;
        }

        // NOTE: Brain not on yet
        TypeResolver::new(
            settings,
            &asts[i],
            module.mod_id,
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

    for i in 0..script_compiler.mods.len() {
        let module = &script_compiler.mods[i];
        if module.src_metadata.is_none() {
            continue;
        }

        ConstraintResolver::new(
            settings,
            &asts[i],
            &interner,
            module.mod_id,
            &mut script_compiler,
        )
        .resolve()
        .unwrap_or_else(|mut diags| reporter.diags.append(&mut diags));
    }

    if !reporter.diags.is_empty() {
        return Err(ScriptError::Semantic(reporter.diags).into());
    }

    Ok(())
}
