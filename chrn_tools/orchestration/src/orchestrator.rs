//TEST: TEST
use chrn_utils::{
    budget::mem_cost::MemoryCost, core_error::ScriptError, id_types::ModuleId,
    source_map::source_diagnostic::Reporter,
};
use compilation::{
    lexer::{Lexer, token::SpannedToken, trivia::Trivia},
    parser::{self, ast::ast_concepts::AstInfo},
    resolvers::{
        constraint_resolver::ConstraintResolver, member_resolver::MemberResolver,
        name_resolver::NamespaceResolver, resolver_env::ResolverEnv, type_resolver::TypeResolver,
    },
    script_compiler::{ScriptCompiler, script_compiler_store::ScriptCompilerStore},
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
    for mod_idx in 0..compiler.mods.len() {
        //TEST: The error messages get worse when they are allowed  to be read with a broken region
        let (mod_id, state) = (ModuleId::new(mod_idx), compiler.mods[mod_idx].state);
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

    // This should be stored internally
    //
    // Creates envs so that resolvers can maintain their state, given the current environment of modules
    let resolver_envs = create_envs(compiler, compiler_store, &compiler_store.asts);

    // Storing this so that the compiler can be borrowed without conflicts and keep resolution incremental
    let mod_len = compiler.mods.len();

    let mut ns_resolver =
        NamespaceResolver::new(&compiler_store.settings, &compiler_store.interner, compiler);

    for i in 0..mod_len {
        // If there is no environment to use then it's not fit for resolution
        // This is a dense array so it works fine
        let current_env = match &resolver_envs[i] {
            Some(env) => env,
            None => continue,
        };

        ns_resolver
            .resolve(&current_env)
            .unwrap_or_else(|mut diags| {
                reporter.diags.append(&mut diags);
            });
    }

    //NOTE: Up to here would be parallelizable since at most they would need to wait asynchronously
    //for the lexer and ast part, then they can do the same here but the next parts would need
    //efficient locking?

    let mut member_diags = MemberResolver::new(
        &compiler_store.settings,
        &resolver_envs,
        &compiler_store.interner,
        compiler,
    )
    .resolve();

    reporter.append_safe(&mut member_diags);

    //TODO: Wrap some of these resolvers into convience functions?

    let mut ty_resolver =
        TypeResolver::new(&compiler_store.settings, &compiler_store.interner, compiler);

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
    if !reporter.diags.is_empty() {
        let mut diags = Vec::new();
        diags.append(&mut reporter.diags);
        return Err(ScriptError::Semantic(diags).into());
    }

    let mut constraint_resolver =
        ConstraintResolver::new(&compiler_store.settings, &compiler_store.interner, compiler);

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
        let mut diags = Vec::new();
        diags.append(&mut reporter.diags);
        return Err(ScriptError::Semantic(diags).into());
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
    let module = &compiler.mods[current_mod_id.id];
    let region = match &module.region_id {
        Some(region_id) => &compiler_store.region_arena.regions[region_id.id as usize],
        None => {
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
    let module = &compiler.mods[current_mod_id.id];
    let region = match &module.region_id {
        Some(region_id) => &compiler_store.region_arena.regions[region_id.id as usize],
        None => {
            // Meaning it's a lib module where None should be found upon any queries
            return None;
        }
    };

    let (ast_info, mut diags) = parser::parse(
        &compiler_store.settings,
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
fn create_envs<'a>(
    compiler: &ScriptCompiler,
    compiler_store: &'a ScriptCompilerStore,
    asts: &'a Vec<Option<AstInfo>>,
) -> Vec<Option<ResolverEnv<'a>>> {
    let mut all_envs = Vec::new();
    for i in 0..compiler.mods.len() {
        let module = &compiler.mods[i];

        let current_region = match &module.region_id {
            Some(region_id) => &compiler_store.region_arena.regions[region_id.id as usize],
            None => {
                all_envs.push(None);
                continue;
            }
        };

        let current_ast = asts[i].as_ref().expect("Has region already");

        //NOTE: pre store envs? Part of cache or store?
        let env = ResolverEnv::new(current_ast, current_region, module.mod_id);
        all_envs.push(Some(env));
    }

    all_envs
}
