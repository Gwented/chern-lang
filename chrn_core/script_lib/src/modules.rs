// TODO: This should be split but not sure what would be best since it is fairly local and small
use std::{fs, path::Path};

pub mod mod_finder;

use chrn_utils::{
    chrn_settings::ChrnSettings,
    config_loader::ChrnConfigLoader,
    core_error::{self, ConfigLoadError},
    id_types::{InternedId, ModuleId, PathId, ScopeId, SourceRegionId, SymbolId},
    intern::{self, Intern},
    source_map::{
        source_diagnostic::{AnnotationKind, DiagnosticLevel, SourceDiagnostic},
        source_region::SourceRegionArena,
        source_span::SourceSpan,
    },
};

use crate::{iyo::file_ops, modules::mod_finder::ModuleFinder, script_compiler::ScriptCompiler};

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

/// A state for modules to be tracked by
// May or may not add more specific states like parsed and such, but this is fine
#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleState {
    Error,
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

/// Helper struct for carrying module data if `main` fails to be loaded within `extract_modules`.
pub struct ModuleInitExtractionError {
    region: Option<SourceRegionArena>,
    cfg_load_err: ConfigLoadError,
}

impl ModuleInitExtractionError {}

//TEST: Lets depending on self recursively as a module happen for now
/// Takes in a path to a `chrn` config file, then recursively resolved all imports associated with
/// the path given in separate modules.
/// THIS IMPLICITLY LOADS CORE
//TODO: Should return an unfinished module state by default where a module may or may not be
//completely loaded. Meaning, this would probably be best returning diagnostics.
pub fn extract_modules(
    path: &Path,
    settings: &ChrnSettings,
    interner: &mut Intern,
) -> Result<(ScriptCompiler, SourceRegionArena), ConfigLoadError> {
    let src = match file_ops::fopen(&path) {
        Ok(f) => f,
        Err(err_msg) => {
            // Interning mangled path id so it can still go into the diagnostic
            let path_id = interner.intern_path(path);
            let src_diag =
                SourceDiagnostic::builder(DiagnosticLevel::Error, err_msg, path_id).build();
            return Err(ConfigLoadError::Module(src_diag));
        }
    };

    let path = path.canonicalize()?;
    let main_path_id = interner.intern_path(&path);

    let mut region_arena: SourceRegionArena = SourceRegionArena::new(Default::default());
    let main_region_id = SourceRegionId::new(0);

    // Using region id before pushing
    let main_region = ChrnConfigLoader::new(main_region_id, src, main_path_id, settings, interner)
        .load_config()?;

    // let region = SourceRegion::new(
    //     self.handle.buffer()[..self.pos + DEFINITION_SIZE].to_vec(),
    //     self.region_id,
    //     lex_start,
    //     Some(serial_start),
    // );

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

            return Err(ConfigLoadError::Module(src_diag));
        }
    };

    let name_id = interner.intern(&file_name);

    // dbg!(str::from_utf8(&main_metadata.src_bytes[..]));
    let main_mod_id = ModuleId::new(0);
    // For now, just give mod finder the dfs search where, it will search and push the vec where
    // needed, so during recursive resolution we can just give the module id.
    //
    // Or ModuleAst

    // This is a Vector relationship stored where, the path id of an import is stored along with a
    // module id. So, we store main, go into main's imports then fill in OR create the module id of
    // unknown imports based off of reserved len(). This works during the recursive process because
    // it MUST look at all imports before ever recursing further, and it reserves it's spot as
    // `None`.
    let mut reserved_mod_ids: Vec<(PathId, ModuleId)> = vec![(main_path_id, main_mod_id)];

    let (bind, main_imports) = ModuleFinder::new(
        &main_region.src_bytes,
        settings,
        &mut reserved_mod_ids,
        &main_region,
        // We don't know the module id for imports. At all.
        main_region.script_start,
        main_region.serial_start,
    )
    .collect_imports(interner)?;

    let main_mod = Module::new(
        name_id,
        ModuleState::Loading,
        main_mod_id,
        main_imports,
        Some(main_region_id),
    );

    region_arena.regions.push(main_region);

    // Will incur borrowing issues unless the main_mod is put in last since the list of it's
    // imports is needed to start recursive process
    //
    // Need to be an Option because a HashMap is not necessary if spots can just be reserved and
    // filled then UNWRAPPED after since we know they're all resolved
    let mut other_mods: Vec<Option<Module>> = Vec::with_capacity(main_mod.imports.len());
    let mut seen: Vec<PathId> = vec![main_path_id];

    resolve_modules(
        &mut reserved_mod_ids,
        &mut seen,
        &mut other_mods,
        &main_mod,
        &mut region_arena,
        settings,
        interner,
    )?;
    // dbg!(reserved_mod_ids, &other_mods, seen);
    // panic!();

    // May change
    // Please change
    // NOT yet
    let mut all_mods: Vec<Module> = Vec::new();
    debug_assert!(
        other_mods.iter().all(|e| e.is_some()),
        "`None` found in other_mods: {other_mods:?}"
    );

    all_mods.push(main_mod);
    for mod_opt in other_mods.drain(..) {
        let known = mod_opt.expect("ModuleId reserving failed");
        all_mods.push(known);
    }

    // all_mods.iter().for_each(|m| {
    //     println!(
    //         "Module \"{}\" nid = {}\nPath: \"{}\" | ModuleId = {:?}\n{:#?}",
    //         interner.search(m.name_id.id as usize),
    //         m.name_id.id,
    //         interner
    //             .search_path(m.src_metadata.as_ref().unwrap().path_id.id as usize)
    //             .display(),
    //         m.mod_id.id,
    //         m.imports
    //     );
    // });

    // for module in &all_mods {
    //     println!(
    //         "Module \"{}\" -> {}",
    //         interner.search(module.name_id.id as usize),
    //         interner
    //             .search_path(module.src_metadata.as_ref().unwrap().path_id.id as usize)
    //             .display()
    //     );
    //     for import in &module.imports {
    //         println!(
    //             "\tImport -> {}",
    //             interner.search_path(import.path_id.id as usize).display()
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
    //         return Err(ConfigLoadError::General(src_diag));
    //     }
    // }

    let compiler = ScriptCompiler::new(bind, all_mods);

    Ok((compiler, region_arena))
}

//WARN: mod_map means that if any have the same identifier then the entire module space is broken
// Need to either error or store differently

// Maybe this has gone a little bit too far
/// This function recursively resolves each import after being given a root module with imports to go off of.
/// `seen`: All imports seen to perform DFS.
/// `modules`: Modules to store during recursive process to return and append to main module.
/// `prev_mod`: The last module so that it's spanning information can be tracked.
/// `mod_map`: Module interned file name -> ModuleId.
fn resolve_modules(
    // Maybe change to vec
    reserved_mod_ids: &mut Vec<(PathId, ModuleId)>,
    seen: &mut Vec<PathId>,
    other_mods: &mut Vec<Option<Module>>,
    prev_mod: &Module,
    region_arena: &mut SourceRegionArena,
    settings: &ChrnSettings,
    interner: &mut Intern,
) -> Result<(), ConfigLoadError> {
    for import in &prev_mod.imports {
        let ImportKind::Source(path_id, path_span) = import.kind else {
            continue;
        };

        // If the path from a given import was seen already then it ensures a stack overflow is
        // avoided by skipping
        if seen.iter().any(|p_id| *p_id == path_id) {
            // dbg!(interner.search_path(path_id));
            // panic!();
            continue;
        }

        seen.push(path_id);

        // Tracks the id of the current module by tracking however many imports were seen, which
        // all represent one module
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

                return Err(ConfigLoadError::Module(src_diag));
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

                return Err(ConfigLoadError::Module(src_diag));
            }
        };

        let sub_region_id = SourceRegionId::new(region_arena.regions.len() as u32);

        //Oh my
        let file_name = match path.file_prefix().map(|n| n.to_str()) {
            Some(Some(p)) => p.to_string(),
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

                    return Err(ConfigLoadError::Module(src_diag));
                }
            }
        };

        let sub_mod_name_id = interner.intern(&file_name);

        // Using region id before pushing
        let sub_region =
            ChrnConfigLoader::new(sub_region_id, src, path_id, settings, interner).load_config()?;

        let (_, sub_imports) = ModuleFinder::new(
            &sub_region.src_bytes,
            settings,
            reserved_mod_ids,
            &sub_region,
            sub_region.script_start,
            sub_region.serial_start,
        )
        .collect_imports(interner)?;

        // Is subtracting 1 because reserved mod includes main.
        let expected_len = reserved_mod_ids.len() - 1;

        // Checking if modules needs to reserve space for more modules. This check is needed
        // because module id registration is tied to when an import is seen, which COULD be later
        // than the module is found recursively, so extra space needs to be reserved in that case.
        if other_mods.len() < expected_len {
            other_mods.resize(expected_len, None);
        }

        // As opposed to how modules are pushed, regions are pushed before recursively descending
        // so no special cases needed for indexing it
        region_arena.regions.push(sub_region);

        let sub_mod = Module::new(
            sub_mod_name_id,
            ModuleState::Loading,
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
            settings,
            interner,
        )?;

        // Needs - 1 so that it fits inside the temporary Vec, but still uses it's actual module
        // id.
        other_mods[current_mod_id.id - 1] = Some(sub_mod);
    }

    Ok(())
}
