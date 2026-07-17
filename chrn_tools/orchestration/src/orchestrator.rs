use chrn_utils::{
    core_error::ScriptError,
    id_types::{ModuleId, SymbolId},
};
use compilation::{
    lexer::{Lexer, token::SpannedToken, trivia::Trivia},
    modules::ModuleState,
    parser::{self, ast::ast_concepts::AstInfo},
    resolvers::{
        constraint_resolver::ConstraintResolver,
        member_resolver::MemberResolver,
        name_resolver::NamespaceResolver,
        resolver_env::{RegistrationEnv, ResolverEnv},
        type_resolver::TypeResolver,
    },
    script_compiler::{
        ScriptCompiler, reporter::Reporter, script_compiler_store::ScriptCompilerStore,
    },
};

use crate::script_compiler_cache::ScriptCompilerCache;

// Should probably just be an option type, assuming the cache won't $#*(%%5j54ojj)jj hi
// pub fn run_all(
//     reporter: &mut Reporter,
//     compiler: &mut ScriptCompiler,
//     compiler_store: &mut ScriptCompilerStore,
// ) -> Result<(), ScriptError> {
// }

// Ok...
// TODO: This should, um
/// Runs every compiler step associated with script
pub fn run_all(
    reporter: &mut Reporter,
    compiler: &mut ScriptCompiler,
    compiler_store: &mut ScriptCompilerStore,
    // Could make this optional
    // More like "Orchestrator"
    //TODO: I don't think this can stay external and maintain usefulness
    compiler_cache: Option<&mut ScriptCompilerCache>,
) -> Result<(), ScriptError> {
    // Doing this first since if modules were identified during the parsing stage any
    // syntax error within another module would not be reportable since the parser failed.

    // Need to separate namespace resolution and type resolver because if the modules namespaces
    // aren't resolved first, then type resolution isn't possible since it could be using types
    // from elsewhere, which are not known yet.
    for i in 0..compiler.mods.len() {
        //TEST: The error messages get worse when they are allowed  to be read with a broken region
        let mod_id = ModuleId::new(i as u32);

        let (toks_opt, trivia_opt) = run_lexer(compiler, compiler_store, &compiler_cache, mod_id);

        let ast_info_opt = if let Some(toks) = &toks_opt {
            let ast_info_opt = run_parser(
                reporter,
                compiler,
                compiler_store,
                &compiler_cache,
                mod_id,
                toks,
            );
            ast_info_opt
        } else {
            None
        };

        // Compiler store stores these as persistent state in the case of any indexing needing to be
        // done.
        // Should it budget here too?
        compiler_store.toks.push(toks_opt);
        compiler_store.trivias.push(trivia_opt);
        compiler_store.asts.push(ast_info_opt);
    }

    // TEST:
    // This should be stored internally
    //
    // Creates envs so that resolvers can maintain their state, given the current environment of modules
    let registration_envs =
        create_registration_envs(compiler, compiler_store, &compiler_store.asts);

    // Storing this so that the compiler can be borrowed without conflicts and keep resolution incremental
    let mod_len = compiler.mods.len();

    let mut ns_resolver =
        NamespaceResolver::new(&compiler_store.cfg, &compiler_store.interner, compiler);

    // Cannot mutate store while looping so ownership is controlled here
    let mut mod_symbols: Vec<Option<Vec<SymbolId>>> = Vec::new();
    for i in 0..mod_len {
        // If there is no environment to use then it's not fit for resolution
        // This is a dense array so it works fine
        let current_env = match &registration_envs[i] {
            Some(env) => env,
            None => {
                mod_symbols.push(None);
                continue;
            }
        };

        let (current_mod_symbols, mut diags) = ns_resolver.resolve(&current_env);
        reporter.append_safe(&mut diags);
        mod_symbols.push(Some(current_mod_symbols));
    }

    // Ownership transfer
    compiler_store.compilation_syms.append(&mut mod_symbols);

    //TEST:
    // Leaving the registration stage and being able to use the later resolver stage env
    let resolver_envs = create_resolver_envs(compiler, compiler_store);

    // if !reporter.diags.is_empty() {
    //     let mut diags = Vec::new();
    //     diags.append(&mut reporter.diags);
    //     return Err(ScriptError::Semantic(diags).into());
    // }

    //NOTE: Up to here would be parallelizable since at most they would need to wait asynchronously
    //for the lexer and ast part, then they can do the same here but the next parts would need
    //efficient locking?

    let mut member_resolver =
        MemberResolver::new(&compiler_store.cfg, &compiler_store.interner, compiler);

    for i in 0..mod_len {
        // If there is no environment to use then it's not fit for resolution
        let current_env = match &resolver_envs[i] {
            Some(env) => env,
            None => continue,
        };

        let mut member_diags = member_resolver.resolve(&current_env);
        reporter.append_safe(&mut member_diags);
    }

    //TODO: Wrap some of these resolvers into convience functions?

    let mut ty_resolver =
        TypeResolver::new(&compiler_store.cfg, &compiler_store.interner, compiler);
    for i in 0..mod_len {
        // If there is no environment to use then it's not fit for resolution
        let current_env = match &resolver_envs[i] {
            Some(env) => env,
            None => continue,
        };

        ty_resolver
            .resolve(&current_env)
            .unwrap_or_else(|mut diags| {
                reporter.append_safe(&mut diags);
            });
    }

    // //TODO: Change this
    // if !reporter.diags.is_empty() {
    //     let mut diags = Vec::new();
    //     diags.append(&mut reporter.diags);
    //     return Err(ScriptError::Semantic(diags).into());
    // }

    let mut constraint_resolver =
        ConstraintResolver::new(&compiler_store.cfg, &compiler_store.interner, compiler);

    for i in 0..mod_len {
        let current_env = match &resolver_envs[i] {
            Some(env) => env,
            None => continue,
        };

        constraint_resolver
            .resolve(&current_env)
            .unwrap_or_else(|mut diags| {
                reporter.append_safe(&mut diags);
            });
    }

    if !reporter.diags.is_empty() {
        return Err(ScriptError::Semantic);
    }

    Ok(())
}

/// * reporter: To store diagnostics
/// * current_mod_id: Current `ModuleId`
/// * compiler: Compiler associated with the current module
/// * compiler_store: Compiler store associated with module
/// * compiler_cache: Optional caching structure
// What about LexerOutput for the Lexer itself to return?
pub fn run_lexer(
    compiler: &ScriptCompiler,
    // Needs to be mutable for lexer
    compiler_store: &mut ScriptCompilerStore,
    // Could make this optional
    // More like "Orchestrator"
    //TODO: I don't think this can stay external and maintain usefulness
    compiler_cache: &Option<&mut ScriptCompilerCache>,
    current_mod_id: ModuleId,
) -> (Option<Vec<SpannedToken>>, Option<Vec<Trivia>>) {
    let module = &compiler.mods[current_mod_id];
    // Skipping any that aren't `Loaded` because it usually leads to duplicated errors from the
    // config loading stage
    let region = match &module.region_id {
        Some(region_id) if module.state == ModuleState::Loaded => {
            &compiler_store.region_arena[*region_id]
        }
        _ => {
            // Meaning it's a lib module where None should be found upon any queries
            return (None, None);
        }
    };

    // Should the lexer just own the interner? This looks weird.
    let (toks, trivia) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
        .tokenize(&mut compiler_store.interner);

    (Some(toks), Some(trivia))
}

/// * reporter: To store diagnostics
/// * current_mod_id: Current `ModuleId`
/// * toks_opt: Tokens which are an Option due to pipelines themselves possibly not knowing if their
/// tokens are `Some` or not.
/// * toks: Tokens associated with the given module
/// * compiler: Compiler associated with the current module
/// * compiler_cache: Optional caching structure
pub fn run_parser(
    reporter: &mut Reporter,
    compiler: &ScriptCompiler,
    // Also needs mutable for lexer
    compiler_store: &mut ScriptCompilerStore,
    // Could make this optional
    // More like "Orchestrator"
    //TODO: I don't think this can stay external and maintain usefulness
    compiler_cache: &Option<&mut ScriptCompilerCache>,
    current_mod_id: ModuleId,
    toks: &[SpannedToken],
) -> Option<AstInfo> {
    let module = &compiler.mods[current_mod_id];
    let region = match &module.region_id {
        Some(region_id) => &compiler_store.region_arena[*region_id],
        None => {
            // Meaning it's a lib module where None should be found upon any queries
            return None;
        }
    };

    let (ast_info, mut diags) = parser::parse(
        &compiler_store.cfg,
        &region,
        &toks,
        &mut compiler_store.interner,
    );

    reporter.append_safe(&mut diags);

    Some(ast_info)
}

/// Creates all environments possible, which is stored aligned with all modules
///
/// This leaves the `asts` structures connected as a reference on purpose to make
/// ownership explicit.
fn create_registration_envs<'a>(
    compiler: &ScriptCompiler,
    compiler_store: &'a ScriptCompilerStore,
    asts: &'a Vec<Option<AstInfo>>,
) -> Vec<Option<RegistrationEnv<'a>>> {
    let mut all_envs = Vec::new();
    for i in 0..compiler.mods.len() {
        let mod_id = ModuleId::new(i as u32);
        let module = &compiler.mods[mod_id];

        let current_region = match &module.region_id {
            Some(region_id) => &compiler_store.region_arena[*region_id],
            None => {
                all_envs.push(None);
                continue;
            }
        };

        // Can fail because if a module's ast creation is stopped, due to something like say, it's
        // state being a broken region. This would panic since the module taht was skipped DOES have
        // a region id, but it's ast creation was simply ignored.
        let current_ast = match asts[i].as_ref() {
            Some(ast) => ast,
            None => {
                all_envs.push(None);
                continue;
            }
        };

        //NOTE: pre store envs? Part of cache or store?
        // Can't !,!.
        let env = RegistrationEnv::new(current_ast, current_region, module.mod_id);
        all_envs.push(Some(env));
    }

    all_envs
}

/// Creates all environments possible, which is stored aligned with all modules
///
/// This leaves the `asts` structures connected as a reference on purpose to make
/// ownership explicit.
fn create_resolver_envs<'a>(
    compiler: &ScriptCompiler,
    compiler_store: &'a ScriptCompilerStore,
) -> Vec<Option<ResolverEnv<'a>>> {
    let mut all_envs = Vec::new();
    for i in 0..compiler.mods.len() {
        let mod_id = ModuleId::new(i as u32);
        let module = &compiler.mods[mod_id];

        let current_region = match &module.region_id {
            Some(region_id) => &compiler_store.region_arena[*region_id],
            None => {
                all_envs.push(None);
                continue;
            }
        };

        // Can fail because if a module's ast creation is stopped, due to something like say, it's
        // state being a broken region. This would panic since the module taht was skipped DOES have
        // a region id, but it's ast creation was simply ignored.
        let current_ast = match compiler_store.asts[i].as_ref() {
            Some(ast) => ast,
            None => {
                all_envs.push(None);
                continue;
            }
        };

        // This is separate because technically these could be individually omitted depending on how
        // compiler created symbols are done in the future. This can't actually fail at this point
        // right now.
        let compilation_syms = match compiler_store.compilation_syms[i].as_ref() {
            Some(ast) => ast,
            None => {
                all_envs.push(None);
                continue;
            }
        };

        //NOTE: pre store envs? Part of cache or store?
        // Can't !,!.
        let env = ResolverEnv::new(current_ast, current_region, module.mod_id, compilation_syms);
        all_envs.push(Some(env));
    }

    all_envs
}
