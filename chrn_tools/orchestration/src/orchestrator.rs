//TEST: TEST
use chrn_utils::{core_error::ScriptError, source_map::source_diagnostic::Reporter};
use compilation::{
    lexer::Lexer,
    parser::{self, ast::AstInfo},
    resolvers::{
        constraint_resolver::ConstraintResolver,
        name_resolver::NamespaceResolver,
        resolver_env::ResolverEnv,
        type_resolver::{TypeResolver, type_context::TypeContext},
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
pub fn run_all_cached(
    reporter: &mut Reporter,
    compiler: &mut ScriptCompiler,
    compiler_store: &mut ScriptCompilerStore,
    // Could make this optional
    // More like "Orchestrator"
    //TODO: I don't think this can stay external and maintain usefulness
    compiler_cache: &mut ScriptCompilerCache,
) -> Result<(), ScriptError> {
    // Doing this first since if modules were identified during the parsing stage any
    // syntax error within another module would not be reportable since the parser failed.

    // Need to separate namespace resolution and type resolver because if the modules namespaces
    // aren't resolved first, then type resolution isn't possible since it could be using types
    // from elsewhere, which are not known yet.
    for mod_idx in 0..compiler.mods.len() {
        let module = &compiler.mods[mod_idx];
        let region = match &module.region_id {
            Some(region_id) => &compiler_store.region_arena.regions[region_id.id as usize],
            // Giving current module id a None ast
            None => {
                // Meaning it's a lib module where None should be found upon any queries
                compiler_store.toks.push(None);
                compiler_store.trivias.push(None);
                continue;
            }
        };

        // Should the lexer just own the interner? This looks weird.
        let (toks, trivia) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut compiler_store.interner);

        let (ast_info, mut diags) = parser::parse(
            &compiler_store.settings,
            &region,
            &toks,
            &mut compiler_store.interner,
        );

        reporter.diags.append(&mut diags);

        // Compiler store stores this as persistent state in the case of any indexing needing to be
        // done.
        compiler_store.toks.push(Some(toks));
        compiler_store.trivias.push(Some(trivia));

        NamespaceResolver::new(
            &compiler_store.settings,
            &ast_info,
            region,
            &compiler_store.interner,
            module.mod_id,
            compiler,
        )
        .resolve()
        .unwrap_or_else(|mut diags| reporter.diags.append(&mut diags));

        // Storing ast for the same reason
        compiler_store.asts.push(Some(ast_info));
    }

    // This should be stored internally
    //
    // Creates envs so that resolvers can maintain their state, given the current environment of modules
    let resolver_envs = create_envs(compiler, compiler_store, &compiler_store.asts);

    // if !reporter.diags.is_empty() {
    //     return Err(ScriptError::Semantic(reporter.diags).into());
    // }

    //TODO: Wrap this operation into a function eventually?

    let mod_len = compiler.mods.len();
    let mut ty_resolver =
        TypeResolver::new(&compiler_store.settings, &compiler_store.interner, compiler);

    for i in 0..mod_len {
        let current_env = match &resolver_envs[i] {
            Some(env) => env,
            None => continue,
        };

        ty_resolver
            .resolve(&current_env)
            .unwrap_or_else(|mut diags| reporter.diags.append(&mut diags));

        // NOTE: Brain on
        // TypeResolver::new(
        //     &self.settings,
        //     env,
        //     &mut ty_ctx,
        //     &self.interner,
        //     &mut self.compiler,
        // )
        // .resolve();
    }

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
            .unwrap_or_else(|mut diags| reporter.diags.append(&mut diags));
        // ConstraintResolver::new(
        //     &self.settings,
        //     &asts[i].as_ref().expect("Has region already"),
        //     region,
        //     &self.interner,
        //     module.mod_id,
        //     &mut self.compiler,
        // )
        // .resolve()
        // .unwrap_or_else(|mut diags| self.reporter.diags.append(&mut diags));
    }

    if !reporter.diags.is_empty() {
        let mut diags = Vec::new();
        diags.append(&mut reporter.diags);
        return Err(ScriptError::Semantic(diags).into());
    }

    Ok(())
}

/// Creates all environments possible, which is stored aligned with all modules
///
/// This leaves the `cache` and `asts` structures connected as a reference on purpose to make
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
