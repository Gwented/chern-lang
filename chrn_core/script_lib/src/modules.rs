use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

pub mod mod_finder;

use chrn_utils::{
    id_types::{InternedId, ModuleId, PathId, ScopeId, SymbolId},
    intern::Intern,
};
use common::{
    chrn_settings::ChrnSettings,
    core_error::{self, ConfigLoadError},
    reporter::{
        self,
        diagnostic::{Area, Diagnostic},
    },
    span::Span,
};

use crate::{
    config_loader::ChrnConfigLoader,
    iyo::file_ops,
    modules::mod_finder::ModuleFinder,
    script_compiler::ScriptCompiler,
    semantic::scopes::{self, ScopeType},
};
//TEST: Relocate reollacl rreellocrelac
#[derive(Debug)]
pub struct Import {
    pub name_id: InternedId,
    pub path_id: PathId,
    pub path_span: Span,
    pub alias_id: Option<InternedId>,
}

impl Import {
    pub fn new(
        name_id: InternedId,
        path_id: PathId,
        path_span: Span,
        alias_id: Option<InternedId>,
        // Maybe "import as" eventually
    ) -> Import {
        Import {
            name_id,
            path_id,
            path_span,
            alias_id,
        }
    }
}

#[derive(Debug, Default)]
pub struct Bind {
    pub path_id: PathId,
    pub path_span: Span,
}

impl Bind {
    pub fn new(path_id: PathId, path_span: Span) -> Bind {
        Bind { path_id, path_span }
    }
}

// What about OUR name?
// What?
// I actually don't know why that's there
// Still don't know
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
    /// Represents the 5 existent scopes
    pub scopes: Vec<ScopeId>,
    /// Flag for checking if a scope exists in the current module more efficiently than manual
    /// iteration.
    pub(crate) held_scopes: u8,
    /// Metadata that exists if the module contains a source file
    // As of right now this represents the difference between a pre-loaded and user space module
    pub src_metadata: Option<ModuleMetadata>,
}

//TEST:
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
        src_metadata: Option<ModuleMetadata>,
    ) -> Module {
        Module {
            name_id,
            mod_id,
            imports,
            held_scopes: scopes::SCOPE_CORE,
            scopes: Vec::new(),
            src_metadata,
        }
    }

    /// Cheap check for if a scope exists with bit-wise operations
    pub fn has_scope(&self, scope_type: ScopeType) -> bool {
        (self.held_scopes & scope_type.to_u8()) != 0
    }
}

#[derive(Debug)]
pub struct ModuleMetadata {
    /// Bytes from chrn config file
    pub src_bytes: Vec<u8>,
    pub path_id: PathId,
    // / Amount of \n within config file so binary search can be done by error reporter
    // pub new_lines: Vec<usize>,
    /// The script language start which can be different depending on if @def is used
    pub script_start: usize,
    /// The serial start which can be None if there is no serialized file within the config file
    pub serial_start: Option<usize>,
}

impl ModuleMetadata {
    pub fn new(
        src_bytes: Vec<u8>,
        path_id: PathId,
        script_start: usize,
        serial_start: Option<usize>,
    ) -> ModuleMetadata {
        ModuleMetadata {
            // new_lines: Vec::new(),
            src_bytes,
            script_start,
            serial_start,
            path_id,
            //TODO: Could be env var
        }
    }
}

//TEST: Lets depending on self recursively as a module happen for now
/// Takes in a path to a `chrn` config file, then recursively resolved all imports associated with
/// the path given in separate modules.
/// THIS IMPLICITLY LOADS STD
pub fn extract_modules(
    // Does this get canonicalized here or earlier..
    path: &Path,
    settings: &ChrnSettings,
    interner: &mut Intern,
) -> Result<ScriptCompiler, ConfigLoadError> {
    let src = match file_ops::fopen(&path) {
        Ok(f) => f,
        Err(err_msg) => {
            let diag = Diagnostic::new(path, err_msg.clone(), None, err_msg, Area::ConfigLoad);
            return Err(ConfigLoadError::Module(diag));
        }
    };

    let path = path.canonicalize()?;
    let path_id = PathId::new(interner.intern_path(&path));

    let main_metadata = ChrnConfigLoader::new(path_id, src, settings, interner).load_config()?;

    // FIX: Aliasing?
    let file_name = match path.file_prefix().map(|n| n.to_str()) {
        Some(Some(p)) => p,
        _ => {
            let core_msg = format!(
                "The path \"{}\" does not have a valid UTF-8 file name usable within the program",
                path.display()
            );

            let diag = Diagnostic::new(&path, core_msg.clone(), None, core_msg, Area::ConfigLoad);

            return Err(ConfigLoadError::Module(diag));
        }
    };

    let name_id = InternedId::new(interner.intern(&file_name));

    // dbg!(str::from_utf8(&main_metadata.src_bytes[..]));
    let (bind, main_imports) = ModuleFinder::new(
        &main_metadata.src_bytes,
        settings,
        path,
        main_metadata.script_start,
        main_metadata.serial_start,
    )
    .collect_imports(interner)?;

    let mod_id = ModuleId::new(0);
    let main_mod = Module::new(name_id, mod_id, main_imports, Some(main_metadata));

    let mut mod_map: HashMap<InternedId, ModuleId> = HashMap::new();
    mod_map.insert(main_mod.name_id, mod_id);

    let mut seen: HashSet<PathId> = HashSet::new();
    seen.insert(path_id);

    // Will incur borrowing issues unless the main_mod is put in last since the list of it's
    // imports is needed to start recursive process
    let mut other_mods: Vec<Module> = Vec::with_capacity(main_mod.imports.len());
    resolve_modules(
        &mut seen,
        &mut other_mods,
        &main_mod,
        &mut mod_map,
        settings,
        interner,
    )?;

    // May change
    // Please change
    let mut all_mods: Vec<Module> = Vec::new();
    all_mods.push(main_mod);
    all_mods.append(&mut other_mods);
    //
    // all_mods.iter().for_each(|m| {
    //     println!(
    //         "Module \"{}\" nid = {}\nPath: \"{}\" | ModuleId = {:?}\n{:#?}",
    //         interner.search(m.name_id.id as usize),
    //         m.name_id.id,
    //         interner.search_path(m.path_id.id as usize).display(),
    //         m.mod_id.id,
    //         m.imports
    //     );
    // });
    //
    // for module in &all_mods {
    //     println!(
    //         "Module \"{}\" -> {}",
    //         interner.search(module.name_id.id as usize),
    //         interner.search_path(module.path_id.id as usize).display()
    //     );
    //     for import in &module.imports {
    //         println!(
    //             "\tImport -> {}",
    //             interner.search_path(import.name_id.id as usize).display()
    //         );
    //     }
    //     println!("_______\n")
    // }

    let compiler = ScriptCompiler::new(bind, mod_map, all_mods);

    Ok(compiler)
}

/// This function recursively resolves each import after being given a root module with imports to go off of.
// Maybe this has gone a little bit too far
/// `seen`: All imports seen to perform DFS.
/// `modules`: Modules to store during recursive process to return and append to main module.
/// `prev_mod`: The last module so that it's spanning information can be tracked.
/// `mod_map`: Module interned file name -> ModuleId.
fn resolve_modules(
    seen: &mut HashSet<PathId>,
    modules: &mut Vec<Module>,
    prev_mod: &Module,
    mod_map: &mut HashMap<InternedId, ModuleId>,
    settings: &ChrnSettings,
    interner: &mut Intern,
) -> Result<(), ConfigLoadError> {
    for import in &prev_mod.imports {
        if seen.contains(&import.path_id) {
            continue;
        }

        // This entire process is performing IO recursively based off of file paths so a failure
        // here would be either an early detailed error, or deterministic system no longer being
        // deterministic
        let prev_metadata = &prev_mod
            .src_metadata
            .as_ref()
            .expect("Infailable currently");

        // Tracks the id of the current module by tracking however many imports were seen, which
        // all represent one module
        let current_mod_id = seen.len();
        seen.insert(import.path_id);

        let path = interner.search_path(import.path_id.id as usize);
        let src = match fs::File::open(path) {
            // Why.
            Ok(_) if path.is_dir() => {
                let core_msg = format!("The path \"{}\" is a directory", path.display());

                let ln_data = reporter::form_err_diag(
                    &prev_metadata.src_bytes,
                    &[import.path_span],
                    settings.can_color,
                );

                let prev_path = interner.search_path(prev_metadata.path_id.id as usize);
                let fmtted_diag = reporter::standardize_err(
                    &core_msg,
                    &ln_data,
                    "",
                    prev_path,
                    settings.can_color,
                );

                let diag = Diagnostic::new(
                    path,
                    core_msg,
                    Some(import.path_span),
                    fmtted_diag,
                    Area::ConfigLoad,
                );

                return Err(ConfigLoadError::Module(diag));
            }
            Ok(f) => f,
            Err(e) => {
                let core_msg =
                    core_error::form_string_from_io_err(&e, path).unwrap_or(e.to_string());

                let ln_data = reporter::form_err_diag(
                    &prev_metadata.src_bytes,
                    &[import.path_span],
                    settings.can_color,
                );

                let prev_path = interner.search_path(prev_metadata.path_id.id as usize);
                let fmtted_diag = reporter::standardize_err(
                    &core_msg,
                    &ln_data,
                    "",
                    prev_path,
                    settings.can_color,
                );

                let diag = Diagnostic::new(
                    path,
                    core_msg,
                    Some(import.path_span),
                    fmtted_diag,
                    Area::ConfigLoad,
                );

                return Err(ConfigLoadError::Module(diag));
            }
        };

        let mod_metadata =
            ChrnConfigLoader::new(import.path_id, src, settings, interner).load_config()?;

        let path = interner.search_path(import.path_id.id as usize);

        //Oh my
        let file_name = match path.file_prefix().map(|n| n.to_str()) {
            Some(Some(p)) => p.to_string(),
            _ => {
                if let Some(name_id) = import.alias_id {
                    interner.search(name_id.id as usize).to_string()
                } else {
                    let core_msg = format!(
                        "The path \"{}\" does not have a valid UTF-8 file name usable within the program. Consider using 'as' to give it an alias if a file name change is not possible.",
                        path.display()
                    );

                    let diag =
                        Diagnostic::new(path, core_msg.clone(), None, core_msg, Area::ConfigLoad);

                    return Err(ConfigLoadError::Module(diag));
                }
            }
        };

        let name_id = InternedId::new(interner.intern(&file_name));

        let origin = interner.search_path(prev_metadata.path_id.id as usize);

        let (_, sub_imports) = ModuleFinder::new(
            &mod_metadata.src_bytes,
            settings,
            origin.to_path_buf(),
            mod_metadata.script_start,
            mod_metadata.serial_start,
        )
        .collect_imports(interner)?;

        let sub_mod = Module::new(
            name_id,
            ModuleId::new(current_mod_id),
            sub_imports,
            Some(mod_metadata),
        );

        if let Some(alias_id) = import.alias_id {
            mod_map.insert(alias_id, ModuleId::new(current_mod_id));
        }

        resolve_modules(seen, modules, &sub_mod, mod_map, settings, interner)?;

        modules.push(sub_mod);
        mod_map.insert(name_id, ModuleId::new(current_mod_id));

        // Modules start off at 0 since the main module can't be inserted before this so + 1 for
        // correct indexing in the final vector
    }

    Ok(())
}
