// TODO: This should be split but not sure what would be best since it is fairly local and small
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

pub mod mod_finder;

use chrn_utils::{
    chrn_settings::ChrnSettings,
    config_loader::ChrnConfigLoader,
    core_error::{self, ConfigLoadError},
    id_types::{InternedId, ModuleId, PathId, ScopeId, SourceRegionId, SymbolId},
    intern::{self, Intern},
    source_map::{
        source_diagnostic::{AnnotationKind, DiagnosticLevel, SourceDiagnostic},
        source_region_data::{SourceRegion, SourceRegionArena},
        source_span::SourceSpan,
    },
};

use crate::{iyo::file_ops, modules::mod_finder::ModuleFinder, script_compiler::ScriptCompiler};

const RESERVED_INTERNED_MODULE_IDENTS: [u32; 1] = [intern::INTERNED_CORE];

//TEST: Relocate reollacl rreellocrelac
#[derive(Debug, Clone)]
pub struct Import {
    pub name_id: InternedId,
    pub kind: ImportKind,
    pub alias_id: Option<InternedId>,
}

impl Import {
    pub fn new(name_id: InternedId, kind: ImportKind, alias_id: Option<InternedId>) -> Import {
        Import {
            name_id,
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
#[derive(Debug)]
pub struct Module {
    /// File name that will be used internally
    pub name_id: InternedId,
    /// It's own module id position
    pub mod_id: ModuleId,
    /// Imports found in the module
    // What if imports were tagged with bit-wise?
    pub imports: Vec<Import>,
    /// Represents the 5 known scopes as well as any local scopes
    pub scopes: Vec<ScopeId>,
    // HashSet maybe
    pub exports: Vec<SymbolId>,
    /// Metadata that exists if the module contains a source file
    // As of right now this represents the difference between a pre-loaded and user space module
    pub src_metadata: Option<SourceRegionId>,
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
        mod_id: ModuleId,
        imports: Vec<Import>,
        //TODO: Convert to explicit kind
        src_metadata: Option<SourceRegionId>,
    ) -> Module {
        Module {
            name_id,
            mod_id,
            imports,
            exports: Vec::new(),
            scopes: Vec::new(),
            src_metadata,
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

//TEST: Lets depending on self recursively as a module happen for now
/// Takes in a path to a `chrn` config file, then recursively resolved all imports associated with
/// the path given in separate modules.
/// THIS IMPLICITLY LOADS CORE
pub fn extract_modules(
    // Does this get canonicalized here or earlier..
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
    let path_id = interner.intern_path(&path);

    let mut region_arena: SourceRegionArena = SourceRegionArena::new(Default::default());
    let main_region_id = SourceRegionId::new(0);

    // Using region id before pushing
    let main_region =
        ChrnConfigLoader::new(main_region_id, src, path_id, settings, interner).load_config()?;

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
                SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, path_id).build();

            return Err(ConfigLoadError::Module(src_diag));
        }
    };

    let name_id = interner.intern(&file_name);

    // dbg!(str::from_utf8(&main_metadata.src_bytes[..]));
    let (bind, main_imports) = ModuleFinder::new(
        &main_region.src_bytes,
        settings,
        &main_region,
        main_region.script_start,
        main_region.serial_start,
    )
    .collect_imports(interner)?;

    let mod_id = ModuleId::new(0);
    let main_mod = Module::new(name_id, mod_id, main_imports, Some(main_region_id));

    let mut mod_map: HashMap<InternedId, ModuleId> = HashMap::new();
    mod_map.insert(main_mod.name_id, mod_id);

    // Vec
    let mut seen: Vec<PathId> = Vec::new();
    seen.push(path_id);

    region_arena.regions.push(main_region);

    // Will incur borrowing issues unless the main_mod is put in last since the list of it's
    // imports is needed to start recursive process
    //
    // Need to be an Option because a HashMap is not necessary if spots can just be reserved and
    // filled then UNWRAPPED after since we know they're all resolved
    let mut other_mods: Vec<Option<Module>> = Vec::with_capacity(main_mod.imports.len());
    resolve_modules(
        &mut seen,
        &mut other_mods,
        &main_mod,
        &mut region_arena,
        &mut mod_map,
        settings,
        interner,
    )?;

    // May change
    // Please change
    // NOT yet
    let mut all_mods: Vec<Module> = Vec::new();
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

    for mod_name_id in mod_map.keys() {
        if RESERVED_INTERNED_MODULE_IDENTS.contains(&mod_name_id.id) {
            let found_mod_id = mod_map[&mod_name_id];
            let found_mod = &all_mods[found_mod_id.id];

            let mod_name = interner.search(*mod_name_id);

            let core_msg = format!("`{mod_name}` is a reserved module identifier");
            let src_diag =
                SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, path_id).build();
            return Err(ConfigLoadError::General(src_diag));
        }
    }

    let compiler = ScriptCompiler::init(bind, mod_map, all_mods);

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
    seen: &mut Vec<PathId>,
    modules: &mut Vec<Option<Module>>,
    prev_mod: &Module,
    region_arena: &mut SourceRegionArena,
    mod_map: &mut HashMap<InternedId, ModuleId>,
    settings: &ChrnSettings,
    interner: &mut Intern,
) -> Result<(), ConfigLoadError> {
    for import in &prev_mod.imports {
        let ImportKind::Source(path_id, path_span) = import.kind else {
            continue;
        };

        if seen.contains(&path_id) {
            if let Some(alias_id) = import.alias_id {
                mod_map.insert(alias_id, ModuleId::new(seen.len() - 1));
            }

            continue;
        }
        // This entire process is performing IO recursively based off of file paths so a failure
        // here would be either an early detailed error
        // let prev_region_id = &prev_mod
        //     .src_metadata
        //     .as_ref()
        //     .expect("Infailable currently");

        // Tracks the id of the current module by tracking however many imports were seen, which
        // all represent one module
        let current_mod_id = seen.len();
        seen.push(path_id);

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

        let path = interner.search_path(path_id);

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
            &sub_region,
            sub_region.script_start,
            sub_region.serial_start,
        )
        .collect_imports(interner)?;

        // As opposed to how modules are pushed, regions are pushed before recursively descending
        // so no special cases needed for indexing it
        region_arena.regions.push(sub_region);

        let sub_mod = Module::new(
            sub_mod_name_id,
            ModuleId::new(current_mod_id),
            sub_imports,
            Some(sub_region_id),
        );

        // Filling in this module's spot
        modules.push(None);

        if let Some(alias_id) = import.alias_id {
            mod_map.insert(alias_id, ModuleId::new(current_mod_id));
        }

        resolve_modules(
            seen,
            modules,
            &sub_mod,
            region_arena,
            mod_map,
            settings,
            interner,
        )?;

        // Needs - 1 so that it fits inside the temporary Vec before being put into a Vec that has
        // the "main" module in it, which would be a + 1, which is what the ModuleId with by
        // default. A, + 1.
        modules[current_mod_id - 1] = Some(sub_mod);
        mod_map.insert(sub_mod_name_id, ModuleId::new(current_mod_id));

        // Modules start off at 0 since the main module can't be inserted before this so + 1 for
        // correct indexing in the final vector
    }

    Ok(())
}
