use std::path::Path;

use chrn_utils::{
    chrn_settings::ChrnSettings,
    core_error::{ModuleInitError, ScriptError},
    intern::Intern,
    source_map::source_diagnostic::Reporter,
};
use compilation::{
    modules,
    script_compiler::{ScriptCompiler, script_compiler_store::ScriptCompilerStore},
};

//ScriptContext? CompilerContext? AbstractCompilerManager?

//TEST:
// 26 MB struct

// Not bit-flags. Stop.
// How.
pub(crate) struct ModuleCache {
    is_name_resolved: bool,
    is_type_resolved: bool,
    is_constraint_resolved: bool,
}

// Should check imports if more is needed to cache
//FIX:
pub struct ScriptCompilerCache {
    // Maybe the compiler should own this since it's less so cache and more so an actual database
    // the compiler needs to survive. Yes, survive.
    // Would probably actually have it's own settings
    // pub(crate) settings: ChrnSettings,
    // pub(crate) spans: Vec<SourceSpan>,
    // Temp. May consider using a single vector that slices indices for each module instead of
    // Vec<Vec>> but not priority right now
    pub(crate) mod_cache: Vec<ModuleCache>,
}

/// Creates
pub fn create_compiler_with_cache(
    path: &Path,
    reporter: &mut Reporter,
    settings: ChrnSettings,
    // I'm so scared
) -> Result<(ScriptCompiler, ScriptCompilerStore, ScriptCompilerCache), ModuleInitError> {
    let interner = Intern::init();
    // let mut spans = SpanArena::new(Vec::new());

    // I'm so scared
    let (compiler, compiler_store, mut diags) = modules::extract_modules(path, settings, interner)?;
    reporter.diags.append(&mut diags);

    let cache = ScriptCompilerCache {
        // spans,
        mod_cache: Default::default(),
    };

    Ok((compiler, compiler_store, cache))
}

impl ScriptCompilerCache {
    // pub fn new(
    //     path: &Path,
    //     settings: ChrnSettings,
    // ) -> Result<ScriptCompilerCache, ModuleInitError> {
    //     let mut interner = Intern::init();
    //     // let mut spans = SpanArena::new(Vec::new());
    //
    //     let (compiler, diags) = modules::extract_modules(path, settings, interner)?;
    //
    //     Ok(ScriptCompilerCache {
    //         // spans,
    //         toks: Default::default(),
    //         trivias: Default::default(),
    //         asts: Default::default(),
    //         mod_cache: Default::default(),
    //     })
    // }

    pub fn is_fully_resolved(&self) -> bool {
        let mut resolved_count = 0;
        for cache in &self.mod_cache {
            // Not wrapper method. We use pure C and MakeFile.
            if cache.is_name_resolved && cache.is_type_resolved && cache.is_constraint_resolved {
                resolved_count += 1;
            }
        }

        resolved_count == self.mod_cache.len()
    }

    // public List<Optional<AstInfo>> get_asts() { return this.asts; }
    // pub fn asts(&self) -> &Vec<Option<AstInfo>> {
    //     &self.asts
    // }

    /// Runs lexer over all modules
    pub fn run_lexer_all(&mut self) -> Result<(), ScriptError> {
        todo!()
        // for
    }

    /// Runs lexer over all modules
    pub fn run_parser_all(&mut self) -> Result<(), ScriptError> {
        todo!()
    }

    // Just
    /// Runs name resolver on all modules
    pub fn run_name_resolver_all(&mut self) -> Result<(), ScriptError> {
        todo!()
    }

    // /// Runs every compiler step on all modules
    // pub fn run_all(&mut self) -> Result<(), ScriptError> {
    //     // Doing this first since if modules were identified during the parsing stage any
    //     // syntax error within another module would not be reportable since the parser failed.
    //     let mut asts: Vec<Option<AstInfo>> = Vec::new();
    //
    //     // Need to separate namespace resolution and type resolver because if the modules namespaces
    //     // aren't resolved first, then type resolution isn't possible since it could be using types
    //     // from elsewhere, which are not known yet.
    //     for mod_idx in 0..self.compiler.mods.len() {
    //         let module = &self.compiler.mods[mod_idx];
    //         let region = match &module.region_id {
    //             Some(region_id) => &self.region_arena.regions[region_id ],
    //             // Giving current module id a None ast
    //             None => {
    //                 // Meaning it's a lib module where None should be found upon any queries
    //                 self.toks.push(None);
    //                 self.trivias.push(None);
    //                 continue;
    //             }
    //         };
    //
    //         let (toks, trivia) =
    //             Lexer::new(region.region_id, &region.src_bytes, region.script_start)
    //                 .tokenize(&mut self.interner);
    //
    //         let ast_info = match parser::parse(&self.settings, &region, &toks, &mut self.interner) {
    //             Ok(info) => info,
    //             Err((unfinished_ast, mut diags)) => {
    //                 self.reporter.diags.append(&mut diags);
    //                 unfinished_ast
    //             }
    //         };
    //
    //         self.toks.push(Some(toks));
    //         self.trivias.push(Some(trivia));
    //
    //         NamespaceResolver::new(
    //             &self.settings,
    //             &ast_info,
    //             region,
    //             &self.interner,
    //             module.mod_id,
    //             &mut self.compiler,
    //         )
    //         .resolve()
    //         .unwrap_or_else(|mut diags| self.reporter.diags.append(&mut diags));
    //
    //         asts.push(Some(ast_info));
    //     }
    //
    //     // if !reporter.diags.is_empty() {
    //     //     return Err(ScriptError::Semantic(reporter.diags).into());
    //     // }
    //
    //     //TODO: Wrap this operation entirely maybe
    //     let mut ty_ctx = TypeContext::new();
    //
    //     let mut ty_resolver = TypeResolver::new(
    //         &self.settings,
    //         &mut ty_ctx,
    //         &self.interner,
    //         &mut self.compiler,
    //     );
    //
    //     for i in 0..self.compiler.mods.len() {
    //         let module = &self.compiler.mods[i];
    //         let current_region = match &module.region_id {
    //             Some(region_id) => &self.region_arena.regions[region_id ],
    //             None => continue,
    //         };
    //
    //         let current_ast = asts[i].as_ref().expect("Has region already");
    //         let env = ResolverEnv::new(current_ast, current_region, module.mod_id);
    //
    //         ty_resolver
    //             .resolve(&env)
    //             .unwrap_or_else(|mut diags| self.reporter.diags.append(&mut diags));
    //
    //         // NOTE: Brain not on yet
    //         // TypeResolver::new(
    //         //     &self.settings,
    //         //     env,
    //         //     &mut ty_ctx,
    //         //     &self.interner,
    //         //     &mut self.compiler,
    //         // )
    //         // .resolve();
    //     }
    //
    //     if !self.reporter.diags.is_empty() {
    //         let mut diags = Vec::new();
    //         diags.append(&mut self.reporter.diags);
    //         return Err(ScriptError::Semantic(diags).into());
    //     }
    //
    //     let mut constraint_resolver =
    //         ConstraintResolver::new(&self.settings, &self.interner, &mut self.compiler);
    //
    //     for i in 0..self.compiler.mods.len() {
    //         let module = &self.compiler.mods[i];
    //         let region = match &module.region_id {
    //             Some(region_id) => &self.region_arena.regions[region_id ],
    //             None => continue,
    //         };
    //
    //         let current_region = match &module.region_id {
    //             Some(region_id) => &self.region_arena.regions[region_id ],
    //             None => continue,
    //         };
    //
    //         let current_ast = asts[i].as_ref().expect("Has region already");
    //
    //         //NOTE: pre store envs? Part of cache?
    //         let env = ResolverEnv::new(current_ast, current_region, module.mod_id);
    //
    //         constraint_resolver
    //             .resolve(&env)
    //             .unwrap_or_else(|mut diags| self.reporter.diags.append(&mut diags));
    //         // ConstraintResolver::new(
    //         //     &self.settings,
    //         //     &asts[i].as_ref().expect("Has region already"),
    //         //     region,
    //         //     &self.interner,
    //         //     module.mod_id,
    //         //     &mut self.compiler,
    //         // )
    //         // .resolve()
    //         // .unwrap_or_else(|mut diags| self.reporter.diags.append(&mut diags));
    //     }
    //
    //     if !self.reporter.diags.is_empty() {
    //         let mut diags = Vec::new();
    //         diags.append(&mut self.reporter.diags);
    //         return Err(ScriptError::Semantic(diags).into());
    //     }
    //
    //     self.asts.append(&mut asts);
    //
    //     Ok(())
    // }
}

// // Maybe this shouldn't take metadata externally
// pub fn interpret_chrn_cfg(path: &Path, settings: &ChrnSettings) -> Result<(), CoreError> {
//     let mut interner = Intern::init();
//     // let mut span_arena: Vec<SourceSpan> = Vec::new();
//
//     // Doing this first since if modules were identified during the parsing stage any
//     // syntax error within another module would not be reportable since the parser failed.
//     let (mut script_compiler, region_arena) =
//         modules::extract_modules(path, settings, &mut interner)?;
//     let mut reporter = Reporter::new();
//
//     //TODO: May have to just make this into an Option<AstInfo>
//     let mut asts: Vec<AstInfo> = Vec::new();
//
//     // Need to separate namespace resolution and type resolver because if the modules namespaces
//     // aren't resolved first, then type resolution isn't possible since it could be using types
//     // from elsewhere, which are not known yet.
//     for mod_idx in 0..script_compiler.mods.len() {
//         let module = &script_compiler.mods[mod_idx];
//         let metadata = match &module.region_id {
//             Some(region_id) => &region_arena.regions[region_id ],
//             None => continue,
//         };
//
//         let (toks, _) = script_lib::lexer::Lexer::new(
//             metadata.region_id,
//             &metadata.src_bytes,
//             metadata.script_start,
//         )
//         .tokenize(&mut interner);
//
//         let ast_info = match script_lib::parser::parse(settings, &metadata, &toks, &mut interner) {
//             Ok(info) => info,
//             Err((unfinished_ast, mut diags)) => {
//                 reporter.diags.append(&mut diags);
//                 unfinished_ast
//             }
//         };
//
//         NamespaceResolver::new(
//             settings,
//             &ast_info,
//             metadata,
//             &interner,
//             module.mod_id,
//             &mut script_compiler,
//         )
//         .resolve()
//         .unwrap_or_else(|mut diags| reporter.diags.append(&mut diags));
//
//         asts.push(ast_info);
//     }
//
//     if !reporter.diags.is_empty() {
//         return Err(ScriptError::Semantic(reporter.diags).into());
//     }
//
//     //FIX: AstId position should be a direct tie, not sequential
//     let mut ty_ctx = TypeContext::new();
//     for i in 0..script_compiler.mods.len() {
//         let module = &script_compiler.mods[i];
//         let metadata = match &module.region_id {
//             Some(region_id) => &region_arena.regions[region_id ],
//             None => continue,
//         };
//
//         // NOTE: Brain not on yet
//         TypeResolver::new(
//             settings,
//             &asts[i],
//             metadata,
//             module.mod_id,
//             &mut ty_ctx,
//             &interner,
//             &mut script_compiler,
//         )
//         .resolve()
//         .unwrap_or_else(|mut diags| reporter.diags.append(&mut diags));
//     }
//
//     if !reporter.diags.is_empty() {
//         return Err(ScriptError::Semantic(reporter.diags).into());
//     }
//
//     for i in 0..script_compiler.mods.len() {
//         let module = &script_compiler.mods[i];
//         let region = match &module.region_id {
//             Some(region_id) => &region_arena.regions[region_id ],
//             None => continue,
//         };
//
//         ConstraintResolver::new(
//             settings,
//             &asts[i],
//             region,
//             &interner,
//             module.mod_id,
//             &mut script_compiler,
//         )
//         .resolve()
//         .unwrap_or_else(|mut diags| reporter.diags.append(&mut diags));
//     }
//
//     if !reporter.diags.is_empty() {
//         return Err(ScriptError::Semantic(reporter.diags).into());
//     }
//
//     Ok(())
// }
