use std::path::Path;

use chrn_utils::{
    chrn_settings::ChrnSettings,
    core_error::{ConfigLoadError, CoreError, ScriptError},
    intern::Intern,
    source_map::{source_diagnostic::Reporter, source_region::SourceRegionArena},
};
use script_lib::{
    modules::{self},
    parser::ast::AstInfo,
    script_compiler::ScriptCompiler,
    semantic::{
        constraint_resolver::ConstraintResolver,
        name_resolver::NamespaceResolver,
        type_resolver::{TypeResolver, type_context::TypeContext},
    },
    token::SpannedToken,
    trivia::Trivia,
};

//ScriptContext? CompilerContext? AbstractCompilerManager?

//TEST:
// 23 MB struct

// Not bit-flags. Stop.
// How.
pub(crate) struct ModuleCache {
    is_name_resolved: bool,
    is_type_resolved: bool,
    is_constraint_resolved: bool,
}

// Should check imports if more is needed to cache
//FIX:
pub struct ChrnManager {
    pub(crate) interner: Intern,
    pub(crate) region_arena: SourceRegionArena,
    pub(crate) settings: ChrnSettings,
    // pub(crate) spans: Vec<SourceSpan>,
    // Temp. May consider using a single vector that slices indices for each module instead of
    // Vec<Vec>> but not priority right now
    pub(crate) toks: Vec<Option<Vec<SpannedToken>>>,
    pub(crate) trivias: Vec<Option<Vec<Trivia>>>,
    pub(crate) asts: Vec<Option<AstInfo>>,
    pub(crate) compiler: ScriptCompiler,
    pub(crate) mod_cache: Vec<ModuleCache>,
}

/// Partial information returns when `ChrnManager` enters a failure state.
pub struct ChrnManagerInitFailure {
    pub interner: Intern,
    pub err: ConfigLoadError,
}

impl ChrnManagerInitFailure {
    fn new(interner: Intern, err: ConfigLoadError) -> ChrnManagerInitFailure {
        ChrnManagerInitFailure { interner, err }
    }
}

impl ChrnManager {
    pub fn init(
        path: &Path,
        settings: ChrnSettings,
    ) -> Result<ChrnManager, ChrnManagerInitFailure> {
        let mut interner = Intern::init();
        // let mut spans = SpanArena::new(Vec::new());

        let (script_compiler, region_arena) =
            match modules::extract_modules(path, &settings, &mut interner) {
                Ok(tuple) => tuple,
                Err(cfg_load_err) => {
                    return Err(ChrnManagerInitFailure::new(interner, cfg_load_err));
                }
            };

        Ok(ChrnManager {
            interner,
            settings,
            // spans,
            toks: Default::default(),
            trivias: Default::default(),
            asts: Default::default(),
            compiler: script_compiler,
            region_arena,
            mod_cache: Default::default(),
        })
    }

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

    // Should this really be done?
    pub fn interner(&self) -> &Intern {
        &self.interner
    }

    // Should this really be done x2?
    pub fn region_arena(&self) -> &SourceRegionArena {
        &self.region_arena
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

    /// Runs every compiler step on all modules
    pub fn run_all(&mut self) -> Result<(), ScriptError> {
        // Doing this first since if modules were identified during the parsing stage any
        // syntax error within another module would not be reportable since the parser failed.
        let mut reporter = Reporter::new();

        let mut asts: Vec<Option<AstInfo>> = Vec::new();

        // Need to separate namespace resolution and type resolver because if the modules namespaces
        // aren't resolved first, then type resolution isn't possible since it could be using types
        // from elsewhere, which are not known yet.
        for mod_idx in 0..self.compiler.mods.len() {
            let module = &self.compiler.mods[mod_idx];
            let region = match &module.src_region_id {
                Some(region_id) => &self.region_arena.regions[region_id.id as usize],
                // Giving current module id a None ast
                None => {
                    // Meaning it's a lib module where None should be found upon any queries
                    self.toks.push(None);
                    self.trivias.push(None);
                    continue;
                }
            };

            let (toks, trivia) = script_lib::lexer::Lexer::new(
                region.region_id,
                &region.src_bytes,
                region.script_start,
            )
            .tokenize(&mut self.interner);

            let ast_info =
                match script_lib::parser::parse(&self.settings, &region, &toks, &mut self.interner)
                {
                    Ok(info) => info,
                    Err((unfinished_ast, mut diags)) => {
                        reporter.diags.append(&mut diags);
                        unfinished_ast
                    }
                };

            self.toks.push(Some(toks));
            self.trivias.push(Some(trivia));

            NamespaceResolver::new(
                &self.settings,
                &ast_info,
                region,
                &self.interner,
                module.mod_id,
                &mut self.compiler,
            )
            .resolve()
            .unwrap_or_else(|mut diags| reporter.diags.append(&mut diags));

            asts.push(Some(ast_info));
        }

        // if !reporter.diags.is_empty() {
        //     return Err(ScriptError::Semantic(reporter.diags).into());
        // }

        let mut ty_ctx = TypeContext::new();
        for i in 0..self.compiler.mods.len() {
            let module = &self.compiler.mods[i];
            let metadata = match &module.src_region_id {
                Some(region_id) => &self.region_arena.regions[region_id.id as usize],
                None => continue,
            };

            // NOTE: Brain not on yet
            TypeResolver::new(
                &self.settings,
                &asts[i].as_ref().expect("Has metadata already"),
                metadata,
                module.mod_id,
                &mut ty_ctx,
                &self.interner,
                &mut self.compiler,
            )
            .resolve()
            .unwrap_or_else(|mut diags| reporter.diags.append(&mut diags));
        }

        if !reporter.diags.is_empty() {
            return Err(ScriptError::Semantic(reporter.diags).into());
        }

        for i in 0..self.compiler.mods.len() {
            let module = &self.compiler.mods[i];
            let metadata = match &module.src_region_id {
                Some(region_id) => &self.region_arena.regions[region_id.id as usize],
                None => continue,
            };

            ConstraintResolver::new(
                &self.settings,
                &asts[i].as_ref().expect("Has metadata already"),
                metadata,
                &self.interner,
                module.mod_id,
                &mut self.compiler,
            )
            .resolve()
            .unwrap_or_else(|mut diags| reporter.diags.append(&mut diags));
        }

        if !reporter.diags.is_empty() {
            return Err(ScriptError::Semantic(reporter.diags).into());
        }

        self.asts.append(&mut asts);

        Ok(())
    }
}

// Maybe this shouldn't take metadata externally
pub fn interpret_chrn_cfg(path: &Path, settings: &ChrnSettings) -> Result<(), CoreError> {
    let mut interner = Intern::init();
    // let mut span_arena: Vec<SourceSpan> = Vec::new();

    // Doing this first since if modules were identified during the parsing stage any
    // syntax error within another module would not be reportable since the parser failed.
    let (mut script_compiler, region_arena) =
        modules::extract_modules(path, settings, &mut interner)?;
    let mut reporter = Reporter::new();

    //TODO: May have to just make this into an Option<AstInfo>
    let mut asts: Vec<AstInfo> = Vec::new();

    // Need to separate namespace resolution and type resolver because if the modules namespaces
    // aren't resolved first, then type resolution isn't possible since it could be using types
    // from elsewhere, which are not known yet.
    for mod_idx in 0..script_compiler.mods.len() {
        let module = &script_compiler.mods[mod_idx];
        let metadata = match &module.src_region_id {
            Some(region_id) => &region_arena.regions[region_id.id as usize],
            None => continue,
        };

        let (toks, _) = script_lib::lexer::Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
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
            metadata,
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
        let metadata = match &module.src_region_id {
            Some(region_id) => &region_arena.regions[region_id.id as usize],
            None => continue,
        };

        // NOTE: Brain not on yet
        TypeResolver::new(
            settings,
            &asts[i],
            metadata,
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
        let region = match &module.src_region_id {
            Some(region_id) => &region_arena.regions[region_id.id as usize],
            None => continue,
        };

        ConstraintResolver::new(
            settings,
            &asts[i],
            region,
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
