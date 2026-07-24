// TODO: The compiler probably shouldn't OWN the `bind` statement, and instead know what module id
// is main so that it knows where to extract the meaningful bind statement.
// TODO: This should be split but not sure what would be best since it is fairly local and small
//TODO: This needs tests
use std::{fs, path::Path};

pub mod mod_finder;

use crate::config_loader::{ConfigLoader, ConfigLoaderOutput};
use chrn_utils::{
    arena::Arena,
    chrn_config::ChrnConfig,
    core_error::{self, ConfigLoadError, ModuleInitError},
    err_codes::ErrorCode,
    files::file_ops,
    id_types::{InternedId, ModuleId, PathId, ScopeId, SourceRegionId, SymbolId},
    intern::{self, Intern},
    source_map::{
        source_diagnostic::{DiagnosticLevel, SourceDiagnostic, annotations::AnnotationKind},
        source_region::SourceRegion,
        source_span::SourceSpan,
    },
};

use crate::{
    modules::mod_finder::ModuleFinder,
    script_compiler::{
        ScriptCompiler, reporter::Reporter, script_compiler_store::ScriptCompilerStore,
    },
};

pub static RESERVED_INTERNED_MODULE_IDENTS: [u32; 1] = [intern::INTERNED_CORE];

//TEST: Relocate reollacl rreellocrelac
#[derive(Debug, Clone)]
pub struct Import {
    pub name_id: InternedId,
    pub mod_id: ModuleId,
    pub kind: ImportKind,
    pub alias_id: Option<InternedId>,
}

impl Import {
    pub const fn new(
        name_id: InternedId,
        mod_id: ModuleId,
        kind: ImportKind,
        alias_id: Option<InternedId>,
    ) -> Import {
        Import {
            name_id,
            mod_id,
            kind,
            alias_id,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ImportKind {
    Source(PathId, SourceSpan),
    Core,
}

#[derive(Debug, Default, Clone)]
pub struct Bind {
    pub path_id: PathId,
    pub path_span: SourceSpan,
}

impl Bind {
    pub const fn new(path_id: PathId, path_span: SourceSpan) -> Bind {
        Bind { path_id, path_span }
    }
}

//TODO:
//Maybe, a kind field that says user or builtin,
//or, a wrapper that has a module that could explicitly represent if it's user or not
//OR maybe src_metadata is actually a kind, which says whether it's user defined or not so it's
//just not a basic nullable field, and actually has meaning
#[derive(Debug, Clone)]
pub struct Module {
    /// File name that will be used internally
    pub name_id: InternedId,
    /// It's own module id position
    pub mod_id: ModuleId,
    /// Imports found in the module
    // What if imports were tagged with bit-wise?
    pub imports: Vec<Import>,
    /// Representation of the module's state
    pub state: ModuleState,
    pub bind: Option<Bind>,
    /// Represents the 5 known scopes as well as any local scopes
    pub scopes: Vec<ScopeId>,
    // HashSet maybe
    pub exports: Vec<SymbolId>,
    /// Metadata that exists if the module contains a source file
    // As of right now this represents the difference between a pre-loaded and user space module
    pub region_id: Option<SourceRegionId>,
}

pub enum ModuleKind {
    User,
    Builtin,
}

/// A state for modules to be tracked by
// May or may not add more specific states like parsed and such, but this is fine
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleState {
    BrokenRegion,
    #[default]
    Loading,
    Loaded,
}

//TEST:
// Kind of what registry is doing right now
// pub struct SectionIndices {
//     neutral: Option<usize>,
//     var: Option<usize>,
//     nest: Option<usize>,
//     complex: Option<usize>,
//     // Ok
//     overrid: Option<usize>,
// }

impl Module {
    pub const fn new(
        name_id: InternedId,
        state: ModuleState,
        mod_id: ModuleId,
        bind: Option<Bind>,
        imports: Vec<Import>,
        //TODO: Convert to explicit kind
        region_id: Option<SourceRegionId>,
    ) -> Module {
        Module {
            name_id,
            mod_id,
            state,
            bind,
            imports,
            exports: Vec::new(),
            scopes: Vec::new(),
            region_id,
        }
    }

    //NOTE: Does not check for alias, but it doesn't change anything since the import with the
    //module's actual name still exists inside the import, with the alias just being second-hand
    pub fn contains_import(&self, other: &Module) -> bool {
        let mut has_import = self.imports.iter().any(|i| i.name_id == other.name_id);

        if !has_import {
            has_import = self.mod_id == other.mod_id;
        }

        has_import
    }
}

//TODO: Methods
/// State needed to build the module graph from main to all sub modules
pub struct ModuleGraph {
    /// Arena
    region_arena: Arena<SourceRegion, SourceRegionId>,
    /// This is a Vector relationship stored where, the path id of an import is stored along with a
    /// `ModuleId`. So, we store main, go into main's imports then fill in OR create the module id of
    /// unknown imports based off of reserved len(). This works during the recursive process because
    /// it MUST look at all imports before ever recursing further, and it reserves it's spot as
    /// `None`.
    reserved_mod_ids: Vec<(PathId, ModuleId)>,
    /// All modules except main
    other_mods: Vec<Option<Module>>,
    /// All paths seen
    seen: Vec<PathId>,
}

impl ModuleGraph {
    pub const fn new(
        region_arena: Arena<SourceRegion, SourceRegionId>,
        reserved_mod_ids: Vec<(PathId, ModuleId)>,
        other_mods: Vec<Option<Module>>,
        seen: Vec<PathId>,
    ) -> ModuleGraph {
        ModuleGraph {
            region_arena,
            reserved_mod_ids,
            other_mods,
            seen,
        }
    }
    // THEY MADE ME DO IT

    pub const fn arena(&self) -> &Arena<SourceRegion, SourceRegionId> {
        &self.region_arena
    }

    pub const fn reserved_mod_ids(&self) -> &Vec<(PathId, ModuleId)> {
        &self.reserved_mod_ids
    }

    pub const fn other_mods(&self) -> &Vec<Option<Module>> {
        &self.other_mods
    }

    pub const fn seen(&self) -> &Vec<PathId> {
        &self.seen
    }
}

// Maybe not
/// Takes in a path to a `chrn` config file, then recursively resolved all imports associated with
/// the path given in separate modules.
///
/// This is a convenience function over `extract_main` and `extract_modules`.
///
/// Returns `Ok` with a `ScriptCompiler`, `ScriptCompilerStore`, and `Vec<SourceDiagnostic>`.
/// Modules in the compiler may or may not have a state which shows they are incomplete, which
/// would mean a diagnostic was emitted. If diagnostics are empty then everything is stable.
///
/// Returns `Err` when the entry point path given experiences an unrecoverable error to where no
/// sort of half state can be processed.
pub fn extract_all_modules(
    path: &Path,
    cfg: ChrnConfig,
    //TODO: Need to figure out how reporter should be treated
    reporter: &mut Reporter,
) -> Result<(ScriptCompiler, ScriptCompilerStore, Vec<SourceDiagnostic>), ModuleInitError> {
    let mut diags = Vec::new();

    let (main_mod, graph, interner, mut new_diags) = extract_main(path, &cfg)?;
    diags.append(&mut new_diags);

    let (compiler, store, mut new_diags) =
        extract_modules(main_mod, graph, reporter, interner, cfg);
    diags.append(&mut new_diags);

    Ok((compiler, store, diags))
}

/// Attempts to extract only the entry point (main)
///
/// On `Ok` returns main module, graph, and possible diagnostics
/// On `Err` returns terminal `ModuleInitError`
pub fn extract_main(
    path: &Path,
    cfg: &ChrnConfig,
) -> Result<(Module, ModuleGraph, Intern, Vec<SourceDiagnostic>), ModuleInitError> {
    let mut interner = Intern::init();

    // All errors regarding the instantiation of main, aside from it's imports, are terminal, since
    // it's the only path that actually gives access to the module tree
    let src = match file_ops::fopen(path) {
        Ok(f) => f,
        Err(err_msg) => {
            // Interning mangled path id so it can still go into the diagnostic
            let path_id = interner.intern_path(path);
            let src_diag =
                SourceDiagnostic::builder(None, DiagnosticLevel::Error, err_msg, path_id).build();
            let cfg_err = ConfigLoadError::Diagnostic(src_diag);

            return Err(ModuleInitError::new(None, interner, cfg_err));
        }
    };

    // Maybe the reporter should just be used
    let mut diags = Vec::new();

    let main_path_id = interner.intern_path(&path);

    let mut region_arena: Arena<SourceRegion, SourceRegionId> = Arena::new();
    let main_region_id = SourceRegionId::new(0);

    // Not sure if main should even recover from this beyond having an existent region
    let (main_region, main_mod_state) =
        match ConfigLoader::new(main_region_id, src, main_path_id, &cfg, &interner).load_config() {
            ConfigLoaderOutput::Success(region) => (region, ModuleState::Loaded),
            // This could be pretty bad to leave here because
            ConfigLoaderOutput::Broken(broken_region, cfg_err) => {
                // Odd handling..
                let diag = match cfg_err {
                    ConfigLoadError::Diagnostic(diag) => diag,
                    ConfigLoadError::IO(io_err) => {
                        let err_str = core_error::form_string_from_io_err(&io_err, path)
                            .unwrap_or(io_err.to_string());
                        SourceDiagnostic::builder(
                            //TODO: Should this have a code?
                            None,
                            DiagnosticLevel::Error,
                            err_str,
                            main_path_id,
                        )
                        .build()
                    }
                };

                diags.push(diag);
                (broken_region, ModuleState::BrokenRegion)
            }
            ConfigLoaderOutput::UnrecoverableErr(cfg_err) => {
                return Err(ModuleInitError::new(None, interner, cfg_err));
            }
        };

    // FIX: Aliasing please
    let file_name = match path.file_prefix().map(|n| n.to_str()) {
        Some(Some(p)) => p,
        _ => {
            let core_msg = format!(
                "The path \"{}\" does not have a valid UTF-8 file name usable within the program",
                path.display()
            );

            let src_diag =
                // Would have code explaining that aliases can circumvent invalid utf8 file names
                SourceDiagnostic::builder(ErrorCode::ImportErr.code().into(), DiagnosticLevel::Error, core_msg, main_path_id)
                    .build();

            // The error itself does not point to anything so this should be fine to omit
            region_arena.push(main_region);

            let cfg_err = ConfigLoadError::Diagnostic(src_diag);
            //NOTE: Not sure what behavior to expect from this since, the region is available, but
            //the cli renderer may or may not properly innately just create a diagnostic that
            //simply has no extra information besides the error msg
            return Err(ModuleInitError::new(Some(region_arena), interner, cfg_err));
        }
    };

    let main_name_id = interner.intern(&file_name);
    let main_mod_id = ModuleId::new(0);

    // This is a Vector relationship stored where, the path id of an import is stored along with a
    // module id. So, we store main, go into main's imports then fill in OR create the module id of
    // unknown imports based off of reserved len(). This works during the recursive process because
    // it MUST look at all imports before ever recursing further, and it reserves it's spot as
    // `None`.
    let mut reserved_mod_ids: Vec<(PathId, ModuleId)> = vec![(main_path_id, main_mod_id)];

    // Maybe don't inherently declare here since it's a little odd to use a returned variable as
    // the main variable to then collect future diagnostics?
    // Maybe not?
    let (bind, main_imports, mut finder_diags) = ModuleFinder::new(
        &main_region.src_bytes,
        cfg,
        &mut reserved_mod_ids,
        &main_region,
        // We don't know the module id for imports. At all.
        main_region.script_start,
        main_region.serial_start,
    )
    .collect_imports(&mut interner);
    diags.append(&mut finder_diags);

    // Pushing main's region
    region_arena.push(main_region);

    // No errors are immediately terminal after this point since the main entry point now exists and
    // can be viewed even if it's the only module that was successfully created
    let main_mod = Module::new(
        main_name_id,
        main_mod_state,
        main_mod_id,
        // Cheap clone
        bind.clone(),
        main_imports,
        Some(main_region_id),
    );

    // Maybe this shouldn't be done, not sure. But the intent of this being returned in such a way
    // is so that callers can decide to use the pieces without loading the rest, not externally
    // manage graph semantics, which is why it uses getters.
    let other_mods: Vec<Option<Module>> = Vec::with_capacity(main_mod.imports.len());
    let seen: Vec<PathId> = vec![main_path_id];
    let graph = ModuleGraph::new(region_arena, reserved_mod_ids, other_mods, seen);

    Ok((main_mod, graph, interner, diags))
}

/// Is intended to pick up where `extract_main` left off
///
/// Returns compiler, it's data storing counter-part, and possible diagnostics
/// No `Result` is returned, but internally there may be many states, such as module states, which
/// might be incomplete, as well as diagnostics.
pub fn extract_modules(
    main_mod: Module,
    mut graph: ModuleGraph,
    reporter: &mut Reporter,
    mut interner: Intern,
    cfg: ChrnConfig,
) -> (ScriptCompiler, ScriptCompilerStore, Vec<SourceDiagnostic>) {
    let mut diags = Vec::new();
    // Oh ok.
    resolve_modules(
        &mut graph,
        &main_mod,
        &mut diags,
        &cfg,
        reporter,
        &mut interner,
    );

    let main_bind = main_mod.bind.clone();
    let mut all_mods: Arena<Module, ModuleId> = vec![main_mod].into();

    // let mut failed_indices: Vec<usize> = Vec::new();
    //TODO: Emit warn if name == `core`
    // If it's the same as core, core still takes precedence, but tooling may interpret it
    // differently, so should reflect that non-deterministic behavior is expected and that the
    // module's name should be changed
    // Before this would need reporting to take care of counting errors and warns instead of just
    // $#%@$#%$%$$ if len() != 0
    let mut next_id = 1;
    for mod_opt in graph.other_mods.drain(..) {
        if let Some(mut inner) = mod_opt {
            // If the `ModuleId` is not sequential due to a `None` module then make sequential
            //
            // This is needed because modules are processed in alignment with their id, which isn't
            // inherently isn't required depending on how a tool uses it, but still retained explicitly here.
            inner.mod_id.id = next_id;
            all_mods.push(inner);
        }

        next_id += 1;
    }

    // More like compilation store
    let compiler_store = ScriptCompilerStore::new(
        cfg,
        graph.region_arena,
        interner,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );

    let compiler = ScriptCompiler::init(main_bind, all_mods);
    (compiler, compiler_store, diags)
}

//WARN: mod_map means that if any have the same identifier then the entire module space is broken
// Need to either error or store differently

// Maybe this has gone a little bit too far
/// This function recursively resolves each import after being given a root module with imports to go off of.
/// `reserved_mod_ids`: All stored K = Path, V = ModuleId, relationships, which were found by
/// `ModuleFinder`'s collect imports method
/// `seen`: All imports seen to perform DFS.
/// `other_mods`: Modules to store during recursive process, which is to be returned and
///  appended with main module.
/// `prev_mod`: The last module so that it's spanning information can be tracked.
/// `diags`: Vector to append any found diagnostics to since errors do not signify immediate
/// failure.
fn resolve_modules(
    graph: &mut ModuleGraph,
    prev_mod: &Module,
    diags: &mut Vec<SourceDiagnostic>,
    cfg: &ChrnConfig,
    reporter: &mut Reporter,
    interner: &mut Intern,
) {
    for import in &prev_mod.imports {
        // Should this be unreachable!'d? If something gets in here and it really isn't a source
        // import, then something major probably happened that's actively wrong
        let ImportKind::Source(path_id, path_span) = import.kind else {
            continue;
        };

        // If the path from a given import was seen already then it skips
        if graph.seen.iter().any(|p_id| *p_id == path_id) {
            continue;
        }

        graph.seen.push(path_id);

        // Tracks the id of the current module by tracking however many imports were seen, which
        // all represent one module
        //
        // This uses expect() because mod_finder is the only collector imports.
        // We are iterating through said imports. Meaning for this iteration to happen mod_finder
        // would have to register the path first.
        let current_mod_id = graph
            .reserved_mod_ids
            .iter()
            .find(|(p_id, _)| *p_id == path_id)
            .map(|(_, m_id)| *m_id)
            .expect("`mod_finder` registration failed");

        let path = interner.search_path(path_id);
        let src = match fs::File::open(path) {
            // Why.
            Ok(_) if path.is_dir() => {
                let core_msg = format!("The path \"{}\" is a directory", path.display());

                let src_diag =
                    SourceDiagnostic::builder(None, DiagnosticLevel::Error, core_msg, path_id)
                        .add_annotation(
                            path_span,
                            AnnotationKind::Primary,
                            "Caused by this import".to_string().into(),
                        )
                        .build();

                diags.push(src_diag);
                cfg.logger().log_err(|| {
                    let prev_name = interner.search(prev_mod.name_id);
                    format!("import dropped for {prev_name} (reason=Import is a path)")
                });
                // Skips import on this error since this means the import is entirely invalid
                continue;
            }
            Ok(f) => f,
            Err(e) => {
                let core_msg =
                    core_error::form_string_from_io_err(&e, path).unwrap_or(e.to_string());

                let src_diag =
                    SourceDiagnostic::builder(None, DiagnosticLevel::Error, core_msg, path_id)
                        .add_annotation(
                            path_span,
                            AnnotationKind::Primary,
                            "Caused by this import".to_string().into(),
                        )
                        .build();

                diags.push(src_diag);
                continue;
            }
        };

        // Creating region id for the current module on this level in the recursive stacke
        let sub_region_id = SourceRegionId::new(graph.region_arena.len() as u32);

        //Oh my
        let file_name = match path.file_prefix().map(|n| n.to_str()).flatten() {
            Some(p) => p.to_string(),
            _ => {
                if let Some(name_id) = import.alias_id {
                    interner.search(name_id).to_string()
                } else {
                    let core_msg = format!(
                        "The path \"{}\" does not have a valid UTF-8 file name usable within the program. Consider using 'as' to give it an alias if a file name change is not possible.",
                        path.display()
                    );

                    let src_diag =
                        //TODO: Alias handling
                        SourceDiagnostic::builder(ErrorCode::ImportErr.code().into(), DiagnosticLevel::Error, core_msg, path_id)
                            .add_annotation(
                                path_span,
                                AnnotationKind::Primary,
                                "Caused by this import".to_string().into(),
                            )
                            .build();

                    diags.push(src_diag);
                    continue;
                }
            }
        };

        let sub_mod_name_id = interner.intern(&file_name);

        // Using region id before pushing
        let (sub_region, sub_state) =
            match ConfigLoader::new(sub_region_id, src, path_id, cfg, interner).load_config() {
                ConfigLoaderOutput::Success(region) => (region, ModuleState::Loaded),
                //FIX: Works but code liability
                ConfigLoaderOutput::Broken(broken_region, cfg_err) => {
                    match cfg_err {
                        ConfigLoadError::Diagnostic(diag) => {
                            diags.push(diag);
                        }
                        ConfigLoadError::IO(e) => {
                            let path = interner.search_path(path_id);
                            let core_msg = core_error::form_string_from_io_err(&e, path)
                                .unwrap_or(e.to_string());
                            let src_diag = SourceDiagnostic::builder(
                                None,
                                DiagnosticLevel::Error,
                                core_msg,
                                path_id,
                            )
                            .add_annotation(path_span, AnnotationKind::Primary, None)
                            .build();

                            diags.push(src_diag);
                        }
                    }

                    (broken_region, ModuleState::BrokenRegion)
                }
                ConfigLoaderOutput::UnrecoverableErr(cfg_err) => {
                    match cfg_err {
                        ConfigLoadError::Diagnostic(diag) => {
                            diags.push(diag);
                        }
                        ConfigLoadError::IO(e) => {
                            let path = interner.search_path(path_id);
                            let core_msg = core_error::form_string_from_io_err(&e, path)
                                .unwrap_or(e.to_string());
                            let src_diag = SourceDiagnostic::builder(
                                None,
                                DiagnosticLevel::Error,
                                core_msg,
                                path_id,
                            )
                            .add_annotation(path_span, AnnotationKind::Primary, None)
                            .build();

                            diags.push(src_diag);
                        }
                    }

                    continue;
                }
            };

        let (bind, sub_imports, mut found_diags) = ModuleFinder::new(
            &sub_region.src_bytes,
            cfg,
            &mut graph.reserved_mod_ids,
            &sub_region,
            sub_region.script_start,
            sub_region.serial_start,
        )
        .collect_imports(interner);

        diags.append(&mut found_diags);

        // Is subtracting 1 because reserved mod includes main.
        let expected_len = graph.reserved_mod_ids.len() - 1;

        //SAFETY
        // Ensuring before any resizing that the total modules never exceed MAX_MODULES
        //
        // Is + 1 because the main module is going to be included inside the actual output so it has
        // to be accounted for or else a max module count of 100 would be exceeded since 100
        // accounts for the other modules + 1 main module.
        if graph.other_mods.len().saturating_add(expected_len + 1)
            > chrn_utils::MAX_MODULES as usize
        {
            let sub_mod_name = interner.search(sub_mod_name_id);
            let core_msg = format!("Exceeded max module amount of {}", chrn_utils::MAX_MODULES);

            let src_diag = SourceDiagnostic::builder(
                ErrorCode::CompilerSafetyLimits.code().into(),
                DiagnosticLevel::Error,
                core_msg,
                path_id,
            )
            .add_note(format!("Last analyzed module was `{sub_mod_name}`"));
            diags.push(src_diag.build());
            reporter.summary.exceeded_max_mods = Some(chrn_utils::MAX_MODULES);
            return;
        }

        // Checking if modules needs to reserve space for more modules. This check is needed
        // because module id registration is tied to when an import is seen, which COULD be later
        // than the module is found recursively, so extra space needs to be reserved in that case.
        if graph.other_mods.len() < expected_len {
            graph.other_mods.resize(expected_len, None);
        }

        // As opposed to how modules are pushed, regions are pushed before recursively descending
        // so no special cases needed for indexing it
        graph.region_arena.push(sub_region);

        let sub_mod = Module::new(
            sub_mod_name_id,
            sub_state,
            current_mod_id,
            bind,
            sub_imports,
            Some(sub_region_id),
        );

        resolve_modules(graph, &sub_mod, diags, cfg, reporter, interner);

        // Needs - 1 so that it fits inside the temporary Vec, but still uses it's actual module
        // id.
        graph.other_mods[(current_mod_id.id - 1) as usize] = Some(sub_mod);
    }
}
