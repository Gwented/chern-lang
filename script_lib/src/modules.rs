use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

pub mod mod_finder;

use chern_core::{
    id_types::{ModuleId, NameId, PathId, SymbolId},
    intern::Intern,
};
use common::{chern_settings::ChernSettings, core_error::ConfigLoadError, reporter};

use crate::{
    config_loader::ChernConfigLoader,
    iyo::file_ops,
    modules::mod_finder::ModuleFinder,
    parser::ast::{Bind, Import},
    script_compiler::ScriptCompiler,
    semantic::{
        representation::Table,
        scopes::{Scope, ScopeManager},
    },
};

// What about OUR name?
// What?
// I actually don't know why that's there
#[derive(Debug)]
pub struct Module {
    /// File name that will be used internally
    pub name_id: NameId,
    /// Actual path used to find the file itself
    pub path_id: PathId,
    /// It's own module id position
    pub mod_id: ModuleId,
    /// Imports found in the module
    pub imports: Vec<Import>,
    pub(crate) scope_manager: ScopeManager,
    pub metadata: ModuleMetadata,
}

impl Module {
    pub fn new(
        name_id: NameId,
        path_id: PathId,
        mod_id: ModuleId,
        imports: Vec<Import>,
        metadata: ModuleMetadata,
    ) -> Module {
        Module {
            name_id,
            path_id,
            mod_id,
            imports,
            scope_manager: ScopeManager::new(),
            metadata,
        }
    }
}

#[derive(Debug)]
pub struct ModuleMetadata {
    /// Bytes from chern config file
    pub src_bytes: Vec<u8>,
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
        script_start: usize,
        serial_start: Option<usize>,
    ) -> ModuleMetadata {
        ModuleMetadata {
            // new_lines: Vec::new(),
            src_bytes,
            script_start,
            serial_start,
            //TODO: Could be env var
        }
    }
}

//TEST: Lets depending on self recursively as a module happen for now
/// Takes in a path to a `chern` config file, then recursively resolved all imports associated with
/// the path given in separate modules.
pub fn extract_modules(
    // Does this get canonicalized here or earlier..
    path: &Path,
    settings: &ChernSettings,
    interner: &mut Intern,
) -> Result<ScriptCompiler, ConfigLoadError> {
    // Maybe the cli should still do something about this since if the first path given
    // isn't valid, it WOULD warrent a basic error
    let src = match file_ops::fopen(path) {
        Ok(f) => f,
        Err(e) => return Err(ConfigLoadError::Module(e)),
    };

    let main_metadata = ChernConfigLoader::new(path, src, settings).load_config()?;

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

    let mod_id = ModuleId::new(0);
    let main_mod = Module::new(name_id, path_id, mod_id, main_imports, main_metadata);

    let mut mod_map: HashMap<NameId, ModuleId> = HashMap::new();
    mod_map.insert(main_mod.name_id, mod_id);

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
        1,
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

    let compiler = ScriptCompiler::new(bind, mod_map, all_mods);

    Ok(compiler)
}

/// This function recursively resolves each import after being given a root module with imports to go off of.
// Maybe this has gone a little bit too far
fn resolve_modules(
    seen: &mut HashSet<PathId>,
    modules: &mut Vec<Module>,
    prev_mod: &Module,
    mod_map: &mut HashMap<NameId, ModuleId>,
    settings: &ChernSettings,
    current_mod_id: usize,
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

        let mod_metadata = ChernConfigLoader::new(path, src, settings).load_config()?;

        //Oh my
        let file_name = match path.file_prefix().map(|n| n.to_str()) {
            Some(Some(p)) => p.to_string(),
            _ => {
                if let Some(name_id) = import.alias_id {
                    interner.search(name_id.id as usize).to_string()
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

        let sub_mod = Module::new(
            name_id,
            import.path_id,
            ModuleId::new(current_mod_id),
            sub_imports,
            mod_metadata,
        );

        if let Some(alias_id) = import.alias_id {
            mod_map.insert(alias_id, ModuleId::new(current_mod_id));
        }

        resolve_modules(
            seen,
            modules,
            &sub_mod,
            mod_map,
            settings,
            current_mod_id + 1,
            interner,
        )?;

        modules.push(sub_mod);
        mod_map.insert(name_id, ModuleId::new(current_mod_id));

        // Modules start off at 0 since the main module can't be inserted before this so + 1 for
        // correct indexing in the final vector
    }

    Ok(())
}
