use std::{collections::VecDeque, io::Read, path::Path};

pub mod mod_finder;
pub mod module_concepts;

use crate::{
    config_loader::{ConfigLoader, ConfigLoaderOutput},
    module::module_concepts::{Import, ImportKind, Module, ModuleGraph, ModuleIdent, ModuleState},
    semantic::checker_helpers::DuplicateTracker,
};
use chrn_utils::{
    arena::Arena,
    chrn_config::{ChrnConfig, chrn_perf::ChrnPerfStage},
    core_error::{self, ConfigLoadError, ModuleInitError},
    err_codes::ErrorCode,
    files::file_ops,
    id_types::{InternedId, ModuleId, PathId, SourceRegionId},
    intern::{self, Intern},
    source_map::{
        source_diagnostic::{
            DiagnosticLevel, SourceDiagnostic, SourceDiagnosticSink, SourceDiagnosticSummary,
            annotations::AnnotationKind,
        },
        source_region::SourceRegion,
    },
};

use crate::{
    module::mod_finder::ModuleFinder,
    script_compiler::{
        ScriptCompiler, reporter::Reporter, script_compiler_store::ScriptCompilerStore,
    },
};

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
pub fn extract_all_modules<R: Read>(
    path: &Path,
    src: R,
    mut cfg: ChrnConfig,
    //TODO: Need to figure out how reporter should be treated
    reporter: &mut Reporter,
) -> Result<(ScriptCompiler, ScriptCompilerStore, SourceDiagnosticSummary), ModuleInitError> {
    let mut summary = SourceDiagnosticSummary::default();

    let (main_mod, graph, interner, new_summary) = extract_main(path, src, &mut cfg)?;
    summary.merge(new_summary);

    let (compiler, store, new_summary) = extract_modules(main_mod, graph, reporter, interner, cfg);
    summary.merge(new_summary);

    Ok((compiler, store, summary))
}

/// Attempts to extract only the entry point (main)
///
/// On `Ok` returns main module, graph, and possible diagnostics
/// On `Err` returns terminal `ModuleInitError`
///
/// NOTE: The corresponding `extract_modules` function expects the pure graph output from this
/// function to be given to it, otherwise the behavior is undefined.
pub fn extract_main<R: Read>(
    main_path: &Path,
    main_src: R,
    cfg: &mut ChrnConfig,
) -> Result<(Module, ModuleGraph, Intern, SourceDiagnosticSummary), ModuleInitError> {
    // A little concerning here because you CAN stop after this point, meaning the timer doesn't
    // matter, but that should probably be factored in externally and just disallow perf entirely.
    // Or at least warn it's inaccurate, or just, separate stages.
    cfg.perf_tracker_mut().start();

    let mut interner = Intern::init();

    // Maybe the reporter should just be used
    let mut summary = SourceDiagnosticSummary::default();

    let main_path_id = interner.intern_path(&main_path);

    let mut region_arena: Arena<SourceRegion, SourceRegionId> = Arena::with_capacity(1);
    let main_region_id = SourceRegionId::new(0);

    // Not sure if main should even recover from this beyond having an existent region
    let (main_region, main_mod_state) =
        match ConfigLoader::new(main_region_id, main_src, main_path_id, &cfg).load_config() {
            ConfigLoaderOutput::Success(region, loader_summary) => {
                summary.merge(loader_summary);
                (region, ModuleState::Loaded)
            }
            // This could be pretty bad to leave here because
            ConfigLoaderOutput::Broken(broken_region, cfg_err) => {
                // Odd handling..
                let diag = match cfg_err {
                    ConfigLoadError::Diagnostic(diag) => diag,
                    ConfigLoadError::IO(io_err) => {
                        let err_str = core_error::form_string_from_io_err(&io_err, main_path)
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

                summary.push_diag(diag);
                (broken_region, ModuleState::BrokenRegion)
            }
            ConfigLoaderOutput::UnrecoverableErr(cfg_err) => {
                return Err(ModuleInitError::new(None, interner, cfg_err));
            }
        };

    // FIX: Aliasing please
    let file_name = match main_path.file_prefix().map(|n| n.to_str()).flatten() {
        Some(p) => p,
        _ => {
            let core_msg = format!(
                "Path \"{}\" does not have a valid UTF-8 file name usable within the program",
                main_path.display()
            );

            let src_diag =
                // Would have code explaining that aliases can circumvent invalid utf8 file names
                SourceDiagnostic::builder(ErrorCode::ImportErr.into(), DiagnosticLevel::Error, core_msg, main_path_id)
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

    let main_name_id = interner.intern(file_name);
    let main_mod_id = ModuleId::new(0);

    // This is a Vector relationship stored where, the path id of an import is stored along with a
    // module id. So, we store main, go into main's imports then fill in OR create the module id of
    // unknown imports based off of reserved len(). This works during the recursive process because
    // it MUST look at all imports before ever recursing further, and it reserves it's spot as
    // `None`.
    let registered_mod_ids: Vec<(PathId, ModuleId)> = vec![(main_path_id, main_mod_id)];

    // Maybe don't inherently declare here since it's a little odd to use a returned variable as
    // the main variable to then collect future diagnostics?
    // Maybe not?
    let (main_bind, main_imports, mut finder_summary) = ModuleFinder::new(
        &main_region.src_bytes,
        cfg,
        &main_region,
        main_region.script_start,
        main_region.serial_start,
    )
    .collect_imports(&mut interner);
    summary.append_summary(&mut finder_summary);

    // Pushing main's region
    region_arena.push(main_region);

    // No errors are immediately terminal after this point since the main entry point now exists and
    // can be viewed even if it's the only module that was successfully created
    let main_mod = Module::new(
        main_name_id,
        main_mod_state,
        main_mod_id,
        main_bind,
        main_imports,
        Some(main_region_id),
    );

    // Maybe this shouldn't be done, not sure. But the intent of this being returned in such a way
    // is so that callers can decide to use the pieces without loading the rest, not externally
    // manage graph semantics, which is why it uses getters.
    // let other_mods: Vec<Option<Module>> = Vec::with_capacity(main_mod.imports.len());

    let seen: Vec<PathId> = vec![main_path_id];
    let graph = ModuleGraph::new(region_arena, registered_mod_ids, seen);

    Ok((main_mod, graph, interner, summary))
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
    mut cfg: ChrnConfig,
) -> (ScriptCompiler, ScriptCompilerStore, SourceDiagnosticSummary) {
    debug_assert_eq!(main_mod.mod_id.id, 0);
    let mut summary = SourceDiagnosticSummary::default();

    let main_bind = main_mod.bind.clone();

    // This ONLY contains verified modules.
    // Modules are only pushed when their all their imports are processed.
    let mut valid_mods: Arena<Module, ModuleId> = Arena::with_capacity(main_mod.imports.len());

    // If this is true then the outer loop must stop (HELLO I AM A LOOP LABEL)
    // Only used when max modules have been exceeded
    let mut should_break_outer = false;
    let max_mods_usize = chrn_utils::MAX_MODULES as usize;

    // Duplicate tracker that will be re-used across different module contexts.
    let mut mod_ident_tracker: DuplicateTracker<ModuleIdent> =
        DuplicateTracker::with_capacities(main_mod.imports.len() + 1, 0);

    // Starting at [root]
    let mut pending_mods: VecDeque<Module> = vec![main_mod].into();

    // Work-list processing of each module
    //
    // If the import from the popped module has not been seen before, mark it as seen so that it is
    // not processed again, create it's module, then push it to the end of the queue so that it's
    // imports can be viewed and processed as modules.
    while let Some(mut importer_mod) = pending_mods.pop_front() {
        let importer_region_id = importer_mod
            .region_id
            .expect("Root module should have region id");
        let importer_path_id = graph.region_arena[importer_region_id].path_id;

        // A module binds its own name, and only its imports can carry an alias, so this is never
        // an alias
        let root_ident = ModuleIdent::new(importer_mod.name_id, false, None);

        mod_ident_tracker.insert_or_store(root_ident);

        // This diner still makes Coke the old-fashioned way
        for imp_idx in 0..importer_mod.imports.len() {
            let imp = importer_mod.imports[imp_idx].clone();

            // We haven't made the compiler yet so no other types of imports should exist
            let ImportKind::UnresolvedSource(ref sp_path_id) = imp.kind else {
                unreachable!();
            };

            // If is_importer, that means it shouldn't count as a duplicate identifier. So, if `main`
            // imports `main` and both mains are the same, no error is emitted because that would be
            // the wrong behavior. This is solely aa self import guard.
            let is_importer = sp_path_id.inner == importer_path_id && imp.sp_alias_id.is_none();

            // The identifier of an import is mutually exclusive. If it has an alias, it can't use
            // the generated identifier, if it only has an identifier it only uses the identifier.
            let (official_ident, official_span) = if let Some(sp_alias_id) = imp.sp_alias_id.clone()
            {
                (sp_alias_id.inner, sp_alias_id.span)
            } else {
                (imp.name_id, sp_path_id.span)
            };

            let mod_ident = ModuleIdent::new(
                official_ident,
                imp.sp_alias_id.is_some(),
                official_span.into(),
            );

            mod_ident_tracker.insert_or_store_unless(mod_ident, || is_importer);

            // If the path from a given import was seen already then it skips
            if graph.seen.binary_search(&sp_path_id.inner).is_ok() {
                // Checking if there is a module id associated with it's path before skipping so
                // that the import can be updated if possible.
                let mod_id_res = graph
                    .registered_mod_ids
                    .binary_search_by_key(&sp_path_id.inner, |(p_id, _)| *p_id)
                    .map(|idx| graph.registered_mod_ids[idx].1);

                // Valid import
                if let Ok(mod_id) = mod_id_res {
                    importer_mod.imports[imp_idx].kind =
                        ImportKind::Source(sp_path_id.clone(), mod_id);
                } else {
                    // Meaning the import was an error source because resolve_module did register a
                    // module id for this import, and it was already seen
                    importer_mod.imports[imp_idx].kind =
                        ImportKind::ErrorSource(sp_path_id.clone());
                }
                continue;
            }

            // It will now continue on the next iteration this import that has already been
            // registered is seen.
            graph.seen.push(sp_path_id.inner);

            // The module produced by this import (Happy birthday)
            let new_mod = match resolve_module(
                imp.clone(),
                official_ident,
                // &importer_mod,
                &mut graph,
                &mut summary,
                &cfg,
                &mut interner,
            ) {
                Ok(m) => {
                    // Need to set the current import to a resolved source or it stays unresolved
                    importer_mod.imports[imp_idx].kind =
                        ImportKind::Source(sp_path_id.clone(), m.mod_id);
                    m
                }
                // Need to transition state from unresolved source to error so future users of this
                // state machine know
                Err(_) => {
                    importer_mod.imports[imp_idx].kind =
                        ImportKind::ErrorSource(sp_path_id.clone());
                    continue;
                }
            };

            // The amount of reserved module ids grows O(modules registered), which is
            // all_mods_len + pending_mods_len, which ensures modules not yet registered are also
            // checked for over-allocation
            let expected_len = graph.registered_mod_ids.len();

            //NOTE: Check if externals (like lsp) can share this if not already
            //SAFETY
            // Ensuring before any resizing that the total modules never exceed MAX_MODULES
            if expected_len > max_mods_usize {
                // Checking if the last processed is in the queue
                let last_processed_name = if let Some(module) = pending_mods.iter().last() {
                    interner.search(module.name_id)
                // Checking if the last processed has been pushed into the final module list
                } else if let Some(module) = valid_mods.iter().last() {
                    interner.search(module.name_id)
                } else {
                    // All checks were exhausted so this was a failure at the entry point
                    interner.search(importer_mod.name_id)
                };

                let core_msg = format!("Exceeded max module count of {}", chrn_utils::MAX_MODULES);

                let src_diag = SourceDiagnostic::builder(
                    ErrorCode::CompilerSafetyLimits.into(),
                    DiagnosticLevel::Error,
                    core_msg,
                    sp_path_id.inner,
                )
                .add_note(format!("Last processed module was `{last_processed_name}`"));

                summary.push_diag(src_diag.build());
                reporter.summary.exceeded_max_mods = Some(chrn_utils::MAX_MODULES);
                should_break_outer = true;
                break;
            }

            // Pushing back so that it's imports can be viewed BEFORE putting it into valid_mods
            pending_mods.push_back(new_mod);
        }

        // This is only met if max modules have been exceeded.
        if should_break_outer {
            if should_break_outer {
                let valid_mods_len = valid_mods.len();
                //SAFETY:
                // Goes through valid module and checks if any module id from one of their imports
                // correspond to an invalid module. All modules from this iteration, including the
                // importer, are dropped.
                //
                // This loop is required because if say module0 imported module1, module1 was a
                // valid module, but then module2 reaches the capacity, that would mean module0
                // already set it's imported associated with module1 as a valid source module. This
                // corrects that by setting it to an error source, which DOESN'T really matter since
                // the caller should be terminating after this anyways but if it ever weren't done
                // this would prevent said bug.
                for valid_mod in valid_mods.iter_mut() {
                    for imp in valid_mod.imports.iter_mut() {
                        if let ImportKind::Source(sp_path_id, m_id) = &imp.kind {
                            if m_id.id as usize >= valid_mods_len {
                                imp.kind = ImportKind::ErrorSource(sp_path_id.clone());
                            }
                        }
                    }
                }
            }
            // It could guarantee the current module, which would mean main is ALWAYS processed, but
            // not sure if that really matters.
            // valid_mods.push(importer_mod);
            break;
        }

        for found in mod_ident_tracker.found_dups.drain(..) {
            let dup_ident = interner.search(found.dup.ident_id);

            // An alias is already present so changes msg
            let add_help = !found.dup.is_alias;

            let core_msg = if found.dup.is_alias {
                format!("Duplicate import alias `{dup_ident}`")
            } else {
                format!("Duplicate identifier `{dup_ident}` generated by this import")
            };

            let imp_span = found
                .dup
                .span
                .expect("Previous loop should guarantee import span");

            let mut builder = SourceDiagnostic::builder(
                ErrorCode::ImportErr.into(),
                DiagnosticLevel::Error,
                core_msg,
                importer_path_id,
            )
            .add_annotation(imp_span, AnnotationKind::Primary, None);

            // The original is the root module when it has no span
            if let Some(original_span) = found.original.span {
                builder = builder.add_annotation(
                    original_span,
                    AnnotationKind::Secondary,
                    "Identifier first generated here".to_string().into(),
                );
            }

            if add_help {
                builder = builder.add_help("Consider giving the import an alias");
            }

            summary.push_diag(builder.build());
        }

        // Clearing current context of what should be considered a duplicate
        mod_ident_tracker.clear();

        // The pending module's imports have all been seen making this the final assignment.
        // Module ids are sequential so this is fine.
        valid_mods.push(importer_mod);
    }

    // Still concerning
    cfg.perf_tracker_mut().stop(ChrnPerfStage::ModuleGraph);

    let compiler = ScriptCompiler::init(main_bind, valid_mods);

    // The store's arrays are dense and indexed by `ModuleId`, so they must cover every module the
    // compiler holds -- the implicit `core` module injected by `init` included, not just the user
    // modules in `valid_mods`.
    let store_capacity = compiler.mods.len();
    let compiler_store = ScriptCompilerStore::new(
        cfg,
        graph.region_arena,
        interner,
        Vec::with_capacity(store_capacity),
        Vec::with_capacity(store_capacity),
        Vec::with_capacity(store_capacity),
        Vec::with_capacity(store_capacity),
    );

    (compiler, compiler_store, summary)
}

/// Takes an import and attempts to turn it into a module
fn resolve_module(
    // The import to turn into a module
    import: Import,
    official_ident: InternedId,
    // Module that imported this module, which triggered this module's processing,
    // importer_mod: &Module,
    graph: &mut ModuleGraph,
    summary: &mut SourceDiagnosticSummary,
    cfg: &ChrnConfig,
    interner: &mut Intern,
) -> Result<Module, ()> {
    //WARN: If there is ever an instance where reserved_mod_ids reserves a module id that is valid,
    //but fails with "Err(())" below, it has broken module id alignment

    let ImportKind::UnresolvedSource(sp_path_id) = import.kind.clone() else {
        unreachable!();
    };
    // Tracks the id of the current module by tracking however many imports were seen, which
    // all represent one module
    //
    // This uses expect() because mod_finder is the only collector imports.
    // We are iterating through said imports. Meaning for this iteration to happen mod_finder
    // would have to register the path first.

    //WARN: This can't fail, if the mod finder found it that means it normalized it, which means
    //it's a real path, which could be a dir I guess
    let path = interner.search_path(sp_path_id.inner);
    let src = match file_ops::fopen(path) {
        Ok(f) => f,
        Err((_, err_msg)) => {
            let src_diag =
                SourceDiagnostic::builder(None, DiagnosticLevel::Error, err_msg, sp_path_id.inner)
                    .add_annotation(
                        sp_path_id.span,
                        AnnotationKind::Primary,
                        "Caused by this import".to_string().into(),
                    )
                    .build();

            summary.push_diag(src_diag);
            return Err(());
        }
    };

    // Creating region id for the current module on this level in the recursive stacke
    let sub_region_id = SourceRegionId::new(graph.region_arena.len() as u32);

    //NOTE: If this were allowed, some odd undefined behavior would exist, which should likely just
    //be omitted entirely. This is the only location where anything "core" in identifier is stopped.
    //The "UB" in this scenario is just that the core module doesn't actually account for the user's
    //"core" module, only the compiler generated one. May change, but probably not.
    //
    //THE BEHAVIOR IS DEFINED IT IS NOT UB,ekAPEKAIOJE$#$#
    if official_ident.id == intern::INTERNED_CORE {
        //TODO: Maybe rename to compiler internals for the error codes to converge
        let core_msg = "`core` is the only identifier that can't be used for modules";
        let builder =
            SourceDiagnostic::builder(None, DiagnosticLevel::Error, core_msg, sp_path_id.inner);
        summary.push_diag(builder.build());
        return Err(());
    }

    // Using region id before pushing
    let (sub_region, sub_state) =
        match ConfigLoader::new(sub_region_id, src, sp_path_id.inner, cfg).load_config() {
            ConfigLoaderOutput::Success(region, new_summary) => {
                summary.merge(new_summary);
                (region, ModuleState::Loaded)
            }
            //FIX: Works but code liability
            ConfigLoaderOutput::Broken(broken_region, cfg_err) => {
                match cfg_err {
                    ConfigLoadError::Diagnostic(diag) => {
                        summary.push_diag(diag);
                    }
                    ConfigLoadError::IO(e) => {
                        let path = interner.search_path(sp_path_id.inner);
                        let core_msg =
                            core_error::form_string_from_io_err(&e, path).unwrap_or(e.to_string());
                        let src_diag = SourceDiagnostic::builder(
                            None,
                            DiagnosticLevel::Error,
                            core_msg,
                            sp_path_id.inner,
                        )
                        .add_annotation(sp_path_id.span, AnnotationKind::Primary, None)
                        .build();

                        summary.push_diag(src_diag);
                    }
                }
                (broken_region, ModuleState::BrokenRegion)
            }
            ConfigLoaderOutput::UnrecoverableErr(cfg_err) => {
                match cfg_err {
                    ConfigLoadError::Diagnostic(diag) => {
                        summary.push_diag(diag);
                    }
                    ConfigLoadError::IO(e) => {
                        let path = interner.search_path(sp_path_id.inner);
                        let core_msg =
                            core_error::form_string_from_io_err(&e, path).unwrap_or(e.to_string());
                        let src_diag = SourceDiagnostic::builder(
                            None,
                            DiagnosticLevel::Error,
                            core_msg,
                            sp_path_id.inner,
                        )
                        .add_annotation(sp_path_id.span, AnnotationKind::Primary, None)
                        .build();

                        summary.push_diag(src_diag);
                    }
                }
                return Err(());
            }
        };

    // The check in the main loop MUST have ensured the module id created here is going to be new
    let current_mod_id = ModuleId::new(graph.registered_mod_ids.len() as u32);
    debug_assert!(
        graph
            .registered_mod_ids
            .iter()
            .all(|(_, m_id)| *m_id != current_mod_id)
    );

    graph
        .registered_mod_ids
        .push((sp_path_id.inner, current_mod_id));

    let (bind, sub_imports, finder_summary) = ModuleFinder::new(
        &sub_region.src_bytes,
        cfg,
        // &mut graph.reserved_mod_ids,
        &sub_region,
        sub_region.script_start,
        sub_region.serial_start,
    )
    .collect_imports(interner);
    summary.merge(finder_summary);

    // As opposed to how modules are pushed, regions are pushed before recursively descending
    // so no special cases needed for indexing it
    graph.region_arena.push(sub_region);

    let sub_mod = Module::new(
        official_ident,
        sub_state,
        current_mod_id,
        bind,
        sub_imports,
        Some(sub_region_id),
    );

    Ok(sub_mod)
}
