// FIX: Being able to use @end by itself with @def is technically a bug since the lexer is the one
// quitting after seeing @end, not the other way around. Either the loader needs to stop caring
// about if @end has an @def, or the emergent feature needs to be removed.
// TODO: This should be split but not sure what would be best since it is fairly local and small
//TODO: This needs tests
use std::{fs, path::Path};

pub mod mod_finder;

use chrn_utils::{
    arena::Arena,
    chrn_config::ChrnConfig,
    core_error::{self, ConfigLoadError, ModuleInitError},
    files::file_ops,
    id_types::{InternedId, ModuleId, PathId, ScopeId, SourceRegionId, SymbolId},
    intern::{self, Intern},
    source_map::{
        source_diagnostic::{DiagnosticLevel, SourceDiagnostic, annotations::AnnotationKind},
        source_region::SourceRegion,
        source_span::SourceSpan,
    },
};
use lang::config_loader::{ConfigLoader, ConfigLoaderOutput};

use crate::{
    modules::mod_finder::ModuleFinder,
    script_compiler::{
        ScriptCompiler, reporter::Reporter, script_compiler_store::ScriptCompilerStore,
        script_compiler_summary::ScriptCompilerSummary,
    },
};

pub const RESERVED_INTERNED_MODULE_IDENTS: [u32; 1] = [intern::INTERNED_CORE];

//TEST: Relocate reollacl rreellocrelac
#[derive(Debug, Clone)]
pub struct Import {
    pub name_id: InternedId,
    pub mod_id: ModuleId,
    pub kind: ImportKind,
    pub alias_id: Option<InternedId>,
}

impl Import {
    pub fn new(
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

#[derive(Debug, Default)]
pub struct Bind {
    pub path_id: PathId,
    pub path_span: SourceSpan,
}

impl Bind {
    pub fn new(path_id: PathId, path_span: SourceSpan) -> Bind {
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
    pub fn new(
        name_id: InternedId,
        state: ModuleState,
        mod_id: ModuleId,
        imports: Vec<Import>,
        //TODO: Convert to explicit kind
        region_id: Option<SourceRegionId>,
    ) -> Module {
        Module {
            name_id,
            mod_id,
            state,
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

//TEST: Let depending on self recursively as a module happen for now
/// Takes in a path to a `chrn` config file, then recursively resolved all imports associated with
/// the path given in separate modules.
///
//// Maybe this is a little redundant
/// Returns `Ok` with a `ScriptCompiler`, `SoruceRegionArena`, and `Vec<SourceDiagnostic>`.
/// Modules in the compiler may or may not have a state which shows they are incomplete, which
/// would mean a diagnostic was emitted. If diagnostics are empty then everything is stable.
///
/// Returns `Err` when the entry point path given experiences an unrecoverable error to where no
/// sort of half state can be processed.
pub fn extract_modules(
    path: &Path,
    cfg: ChrnConfig,
    reporter: &mut Reporter,
    mut interner: Intern,
) -> Result<(ScriptCompiler, ScriptCompilerStore, Vec<SourceDiagnostic>), ModuleInitError> {
    // All errors regarding the instantiation of main, aside from it's imports, are terminal, since
    // it's the only path that actually gives access to the module tree
    let src = match file_ops::fopen(path) {
        Ok(f) => f,
        Err(err_msg) => {
            // Interning mangled path id so it can still go into the diagnostic
            let path_id = interner.intern_path(path);
            let src_diag =
                SourceDiagnostic::builder(DiagnosticLevel::Error, err_msg, path_id).build();
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
                        SourceDiagnostic::builder(DiagnosticLevel::Error, err_str, main_path_id)
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

    // FIX: Aliasing?
    let file_name = match path.file_prefix().map(|n| n.to_str()) {
        Some(Some(p)) => p,
        _ => {
            let core_msg = format!(
                "The path \"{}\" does not have a valid UTF-8 file name usable within the program",
                path.display()
            );

            let src_diag =
                SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, main_path_id).build();

            let cfg_err = ConfigLoadError::Diagnostic(src_diag);
            //NOTE: Not sure what behavior to expect from this since, the region is available, but
            //the cli renderer may or may not properly innately just create a diagnostic that
            //simply has no extra information besides the error msg
            return Err(ModuleInitError::new(Some(region_arena), interner, cfg_err));
        }
    };

    let name_id = interner.intern(&file_name);

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
        &cfg,
        &mut reserved_mod_ids,
        &main_region,
        // We don't know the module id for imports. At all.
        main_region.script_start,
        main_region.serial_start,
    )
    .collect_imports(&mut interner);
    diags.append(&mut finder_diags);

    // No errors are immediately terminal after this point since the main entry point now exists and
    // can be viewed even if it's the only module that was successfully created
    let main_mod = Module::new(
        name_id,
        main_mod_state,
        main_mod_id,
        main_imports,
        Some(main_region_id),
    );

    region_arena.push(main_region);

    // Will incur borrowing issues unless the main_mod is put in last since the list of it's
    // imports is needed to start recursive process
    //
    // Need to be an Option because a HashMap is not necessary if spots can just be reserved and
    // filled then UNWRAPPED after since we know they're all resolved
    let mut other_mods: Vec<Option<Module>> = Vec::with_capacity(main_mod.imports.len());
    let mut seen: Vec<PathId> = vec![main_path_id];

    // Oh ok.
    resolve_modules(
        &mut reserved_mod_ids,
        &mut seen,
        &mut other_mods,
        &main_mod,
        &mut region_arena,
        &mut diags,
        &cfg,
        reporter,
        &mut interner,
    );

    // dbg!(reserved_mod_ids, &other_mods, seen);
    // panic!();

    // May change
    // Please change
    // NOT yet
    let mut all_mods: Arena<Module, ModuleId> = vec![main_mod].into();
    // debug_assert!(
    //     other_mods.iter().all(|e| e.is_some()),
    //     "`None` found in other_mods: {other_mods:?}"
    // );

    // let mut failed_indices: Vec<usize> = Vec::new();
    let mut next_id = 1;
    for mod_opt in other_mods.drain(..) {
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

    // all_mods.iter().for_each(|m| {
    //     let region_id = *m.region_id.as_ref().unwrap();
    //     let path = interner.search_path(region_arena.get_region(region_id).unwrap().path_id);
    //     println!(
    //         "Module \"{}\" nid = {}\nPath: \"{}\" | ModuleId = {:?}\n{:#?}",
    //         interner.search(m.name_id),
    //         m.name_id.id,
    //         path.display(),
    //         m.mod_id.id,
    //         m.imports
    //     );
    // });
    //
    // for module in &all_mods {
    //     println!(
    //         "Module \"{}\" -> {}",
    //         interner.search(module.name_id ),
    //         interner
    //             .search_path(module.src_metadata.as_ref().unwrap().path_id )
    //             .display()
    //     );
    //     for import in &module.imports {
    //         println!(
    //             "\tImport -> {}",
    //             interner.search_path(import.path_id ).display()
    //         );
    //     }
    //     println!("_______\n")
    // }

    // I don't THINK this causes issues since lookups are by symbol, not identifier. Should maybe
    // warn if needed.
    // for mod_name_id in seen_map.iter().map(|(_, mod_id)| mod_id) {
    //     if RESERVED_INTERNED_MODULE_IDENTS.contains(&mod_name_id.id) {
    //         let mod_name = interner.search(*mod_name_id);
    //         let mod_id = seen_map[mod_name_id];
    //         let err_mod = &all_mods[mod_id.id];
    //
    //         let region_id = err_mod
    //             .region_id
    //             .expect("Should only have source created modules before initializing compiler");
    //         let region = region_arena.extract_region(region_id);
    //
    //         let core_msg = format!("`{mod_name}` is a reserved module identifier");
    //
    //         // File system gui!
    //         let src_diag =
    //             SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, region.path_id).build();
    //         return Err(ConfigLoadError::Diagnostic(src_diag));
    //     }
    // }

    // More like compilation store
    let compiler_store = ScriptCompilerStore::new(
        cfg,
        region_arena,
        interner,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );

    let compiler = ScriptCompiler::init(bind, all_mods);

    Ok((compiler, compiler_store, diags))
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
    reserved_mod_ids: &mut Vec<(PathId, ModuleId)>,
    seen: &mut Vec<PathId>,
    other_mods: &mut Vec<Option<Module>>,
    prev_mod: &Module,
    region_arena: &mut Arena<SourceRegion, SourceRegionId>,
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
        if seen.iter().any(|p_id| *p_id == path_id) {
            continue;
        }

        seen.push(path_id);

        // Tracks the id of the current module by tracking however many imports were seen, which
        // all represent one module
        //
        // This uses expect() because mod_finder is the only collector imports.
        // We are iterating through said imports. Meaning for this iteration to happen mod_finder
        // would have to register the path first.
        let current_mod_id = reserved_mod_ids
            .iter()
            .find(|(p_id, _)| *p_id == path_id)
            .map(|(_, m_id)| *m_id)
            .expect("`mod_finder` registration failed");

        let path = interner.search_path(path_id);
        let src = match fs::File::open(path) {
            // Why.
            Ok(_) if path.is_dir() => {
                let core_msg = format!("The path \"{}\" is a directory", path.display());

                let src_diag = SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, path_id)
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

                let src_diag = SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, path_id)
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
        let sub_region_id = SourceRegionId::new(region_arena.len() as u32);

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

                    //WARN: This didn't use a span before so could be an issue
                    let src_diag =
                        SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, path_id)
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
                            // FIX:
                            // If this case is met, an index out of bounds error occurs inside of the
                            // cli because this uses the region id of the source itself, which is valid
                            // because the source does exist, but the region is never pushed, hence it's
                            // still out of bounds despite being correct.
                            let path = interner.search_path(path_id);
                            let core_msg = core_error::form_string_from_io_err(&e, path)
                                .unwrap_or(e.to_string());
                            //FIX: THIS MAY NOT BE COVERED
                            let src_diag = SourceDiagnostic::builder(
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

        let (_, sub_imports, mut found_diags) = ModuleFinder::new(
            &sub_region.src_bytes,
            cfg,
            reserved_mod_ids,
            &sub_region,
            sub_region.script_start,
            sub_region.serial_start,
        )
        .collect_imports(interner);

        diags.append(&mut found_diags);

        // Is subtracting 1 because reserved mod includes main.
        let expected_len = reserved_mod_ids.len() - 1;

        //SAFETY
        // Ensuring before any resizing that the total modules never exceed MAX_MODULES
        //
        // Is + 1 because the main module is going to be included inside the actual output so it has
        // to be accounted for or else a max module count of 100 would be exceeded since 100
        // accounts for the other modules + 1 main module.
        if other_mods.len().saturating_add(expected_len + 1) > chrn_utils::MAX_MODULES as usize {
            let sub_mod_name = interner.search(sub_mod_name_id);
            let core_msg = format!("Exceeded max module amount of {}", chrn_utils::MAX_MODULES);

            let src_diag = SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, path_id)
                .add_note(format!("Last analyzed module was `{sub_mod_name}`"));
            diags.push(src_diag.build());
            reporter.summary.exceeded_max_mods = Some(chrn_utils::MAX_MODULES);
            return;
        }

        // Checking if modules needs to reserve space for more modules. This check is needed
        // because module id registration is tied to when an import is seen, which COULD be later
        // than the module is found recursively, so extra space needs to be reserved in that case.
        if other_mods.len() < expected_len {
            other_mods.resize(expected_len, None);
        }

        // As opposed to how modules are pushed, regions are pushed before recursively descending
        // so no special cases needed for indexing it
        region_arena.push(sub_region);

        let sub_mod = Module::new(
            sub_mod_name_id,
            ModuleState::Loaded,
            current_mod_id,
            sub_imports,
            Some(sub_region_id),
        );

        resolve_modules(
            reserved_mod_ids,
            seen,
            other_mods,
            &sub_mod,
            region_arena,
            diags,
            cfg,
            reporter,
            interner,
        );

        // Needs - 1 so that it fits inside the temporary Vec, but still uses it's actual module
        // id.
        other_mods[(current_mod_id.id - 1) as usize] = Some(sub_mod);
    }
}
