use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use common::{
    config_loader::ChernConfigLoader,
    core_error::ConfigLoadError,
    intern::Intern,
    metadata::{ChernSettings, ModuleMetadata},
    reporter,
    symbols::{ModuleId, NameId, PathId},
};
pub mod mod_finder;

use crate::{
    iyo::file_ops,
    modules::mod_finder::ModuleFinder,
    parser::ast::{Bind, Import},
    semantic::representation::Table,
};

pub struct Program {
    pub bind: Option<Bind>,
    pub mod_map: HashMap<NameId, ModuleId>,
    pub mods: Vec<Module>,
}

impl Program {
    pub fn new(
        bind: Option<Bind>,
        mod_map: HashMap<NameId, ModuleId>,
        mods: Vec<Module>,
    ) -> Program {
        Program {
            bind,
            mod_map,
            mods,
        }
    }
}

// What about OUR name?
// What?
// I actually don't know why that's there
#[derive(Debug)]
pub struct Module {
    /// File name that will be used internally
    pub name_id: NameId,
    /// Actual path used to find the file itself
    pub path_id: PathId,
    /// Imports found in the module
    pub imports: Vec<Import>,
    pub metadata: ModuleMetadata,
    pub table: Table,
}

impl Module {
    pub fn new(
        name_id: NameId,
        path_id: PathId,
        imports: Vec<Import>,
        metadata: ModuleMetadata,
    ) -> Module {
        Module {
            name_id,
            path_id,
            imports,
            metadata,
            table: Table::new(),
        }
    }
}

//TEST: Lets depending on self recursively as a module happen for now
/// Takes in a path to a `chern` config file, then recursively resolved all imports associated with
/// the path given in separate modules.
pub fn extract_modules(
    path: &Path,
    settings: &ChernSettings,
    interner: &mut Intern,
) -> Result<Program, ConfigLoadError> {
    // Maybe the cli should still do something about this since if the first path given
    // isn't valid, it WOULD warrent a basic error
    let src = match file_ops::fopen(path) {
        Ok(f) => f,
        Err(e) => return Err(ConfigLoadError::Module(e)),
    };

    let main_metadata = ChernConfigLoader::new(path, src).load_config()?;

    // Get's actual file name so that any reference such as, "global.CONSTANT_VALUE" can be
    // accessed by using the file's name, which has to be valid UTF-8 unlike it's path.
    let file_name = match path.file_prefix().map(|n| n.to_str()) {
        Some(Some(p)) => p,
        _ => {
            let msg = format!(
                "The path \"{}\" does not have a valid UTF-8 file name usable within the program",
                path.display()
            );

            return Err(ConfigLoadError::Module(msg));
        }
    };

    let name_id = NameId::new(interner.intern(&file_name));
    let path_id = PathId::new(interner.intern_path(path));

    let (bind, main_imports) = ModuleFinder::new(
        &main_metadata.src_bytes,
        main_metadata.script_start,
        main_metadata.serial_start,
    )
    .collect_imports(interner);

    let main_mod = Module::new(name_id, path_id, main_imports, main_metadata);

    let mut mod_map: HashMap<NameId, ModuleId> = HashMap::new();
    mod_map.insert(main_mod.name_id, ModuleId::new(0));

    let mut seen: HashSet<PathId> = HashSet::new();
    seen.insert(main_mod.path_id);

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

    // Module viewing command
    // Module hierarchy command, dependencies, extended classes, Springboot support
    // for module in &all_mods {
    //     println!(
    //         "Module \"{}\" -> {}",
    //         interner.search(module.name_id.id as usize),
    //         interner.search_path(module.path_id.id as usize).display()
    //     );
    //     for path_id in &module.imports {
    //         println!(
    //             "\tImport -> {}",
    //             interner.search_path(path_id.id as usize).display()
    //         );
    //     }
    //     println!("_______\n")
    // }
    // panic!();

    let program = Program::new(bind, mod_map, all_mods);

    Ok(program)
}

/// This function recursively resolves each import after being given a root module with imports to go off of.
// Maybe this has gone a little bit too far
fn resolve_modules(
    seen: &mut HashSet<PathId>,
    modules: &mut Vec<Module>,
    prev_mod: &Module,
    mod_map: &mut HashMap<NameId, ModuleId>,
    settings: &ChernSettings,
    interner: &mut Intern,
) -> Result<(), ConfigLoadError> {
    for import in &prev_mod.imports {
        if seen.contains(&import.path_id) {
            continue;
        }

        seen.insert(import.path_id);

        let path = interner.search_path(import.path_id.id as usize);
        let src = match fs::File::open(path) {
            // Why.
            Ok(_) if path.is_dir() => {
                let msg = format!("The path \"{}\" is a directory", path.display());

                let ln_data = reporter::form_err_diag(
                    &prev_mod.metadata.src_bytes,
                    &[import.path_span],
                    settings.can_color,
                );
                let prev_path = interner.search_path(prev_mod.path_id.id as usize);
                let full_msg =
                    reporter::standardize_err(&msg, &ln_data, "", prev_path, settings.can_color);

                return Err(ConfigLoadError::Module(full_msg));
            }
            Ok(f) => f,
            Err(e) => {
                let msg = match e.kind() {
                    std::io::ErrorKind::NotFound => {
                        format!("Could not find the file \"{}\"", path.display())
                    }
                    std::io::ErrorKind::PermissionDenied => {
                        format!("No permission to access file \"{}\"", path.display())
                    }
                    std::io::ErrorKind::IsADirectory => {
                        format!("The path \"{}\" is a directory", path.display())
                    }
                    e => format!("{e}"),
                };

                let ln_data = reporter::form_err_diag(
                    &prev_mod.metadata.src_bytes,
                    &[import.path_span],
                    settings.can_color,
                );
                let prev_path = interner.search_path(prev_mod.path_id.id as usize);
                let full_msg =
                    reporter::standardize_err(&msg, &ln_data, "", prev_path, settings.can_color);

                return Err(ConfigLoadError::Module(full_msg));
            }
        };

        let mod_metadata = ChernConfigLoader::new(path, src).load_config()?;

        //Oh my
        let file_name = match path.file_prefix().map(|n| n.to_str()) {
            Some(Some(p)) => p.to_string(),
            _ => {
                if let Some(alias_id) = import.alias_id {
                    interner.search(alias_id.id as usize).to_string()
                } else {
                    let msg = format!(
                        "The path \"{}\" does not have a valid UTF-8 file name usable within the program. Consider using 'as' give it an alias if a file name change is not possible.",
                        path.display()
                    );
                    return Err(ConfigLoadError::Module(msg));
                }
            }
        };

        let name_id = NameId::new(interner.intern(&file_name));

        let (_, sub_imports) = ModuleFinder::new(
            &mod_metadata.src_bytes,
            mod_metadata.script_start,
            mod_metadata.serial_start,
        )
        .collect_imports(interner);

        let sub_mod = Module::new(name_id, import.path_id, sub_imports, mod_metadata);

        resolve_modules(seen, modules, &sub_mod, mod_map, settings, interner)?;

        // Modules start off at 0 since the main module can't be inserted before this so + 1 for
        // correct indexing in the final vector
        mod_map.insert(name_id, ModuleId::new(modules.len() + 1));
        modules.push(sub_mod);
    }

    Ok(())
}
