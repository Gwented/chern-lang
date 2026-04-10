use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

use common::{
    config_loader::ChernConfigLoader,
    core_error::{ConfigLoadError, ScriptError},
    intern::Intern,
    metadata::ChernMetadata,
    symbols::{ModuleId, NameId, PathId},
};
pub mod mod_finder;

use crate::{modules::mod_finder::ModuleFinder, semantic::representation::Table};

pub struct Program {
    pub bind: Option<PathId>,
    pub mod_map: HashMap<NameId, ModuleId>,
    pub mods: Vec<Module>,
}

impl Program {
    pub fn new(
        bind: Option<PathId>,
        mod_map: HashMap<NameId, ModuleId>,
        mods: Vec<Module>,
    ) -> Program {
        Program {
            bind,
            mod_map,
            mods,
        }
    }

    pub fn set_bind(&mut self, path_id: PathId) {
        self.bind = Some(path_id);
    }
}

// What about OUR name?
#[derive(Debug)]
pub struct Module {
    /// File name that will be used internally
    pub name_id: NameId,
    /// Actual path used to find the file itself
    pub path_id: PathId,
    pub imports: Vec<PathId>,
    pub metadata: ChernMetadata,
    pub table: Table,
}

impl Module {
    pub fn new(
        file_id: NameId,
        path_id: PathId,
        imports: Vec<PathId>,
        metadata: ChernMetadata,
    ) -> Module {
        Module {
            name_id: file_id,
            path_id,
            imports,
            metadata,
            table: Table::new(),
        }
    }
}

//TEST: Lets depending on self recursively as a module happen for now
pub fn extract_modules(path: &Path, interner: &mut Intern) -> Result<Program, ConfigLoadError> {
    let src = fs::File::open(path)?;
    let main_metadata = ChernConfigLoader::new(path, src).load_config()?;

    // Get's actual file name so that any reference such as, "global.CONSTANT_VALUE" can be
    // accessed by using the file's name, which has to be valid UTF-8 unlike it's path.
    let file_name = match path.file_prefix().map(|n| n.to_str()) {
        Some(Some(p)) => p.to_string(),
        _ => {
            let msg = format!(
                "The path \"{}\" does not have a valid UTF-8 file name usable within the program",
                path.display()
            );
            return Err(ConfigLoadError::Module(msg));
        }
    };

    let file_id = NameId::new(interner.intern(&file_name));
    let path_id = PathId::new(interner.intern_path(path));

    let main_imports = ModuleFinder::new(
        &main_metadata.src_bytes,
        main_metadata.script_start,
        main_metadata.serial_start,
    )
    .collect_imports(interner);

    let main_mod = Module::new(file_id, path_id, main_imports, main_metadata);

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
        &main_mod.imports,
        &mut mod_map,
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
    //         "Module -> {}",
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

    let program = Program::new(None, mod_map, all_mods);

    Ok(program)
}

/// This function recursively resolves each import after being given a root module with imports to go off of.
fn resolve_modules(
    seen: &mut HashSet<PathId>,
    modules: &mut Vec<Module>,
    imports: &Vec<PathId>,
    mod_map: &mut HashMap<NameId, ModuleId>,
    interner: &mut Intern,
) -> Result<(), ConfigLoadError> {
    for path_id in imports {
        if seen.contains(path_id) {
            continue;
        }

        seen.insert(*path_id);

        let path = interner.search_path(path_id.id as usize);
        let src = fs::File::open(path)?;
        let metadata = ChernConfigLoader::new(path, src).load_config()?;

        //Oh my
        let file_name = match path.file_prefix().map(|n| n.to_str()) {
            Some(Some(p)) => p.to_string(),
            _ => {
                let msg = format!(
                    "The path \"{}\" does not have a valid UTF-8 file name usable within the program",
                    path.display()
                );
                return Err(ConfigLoadError::Module(msg));
            }
        };

        let name_id = NameId::new(interner.intern(&file_name));
        // Modules start off at 0 since the main module can't be inserted before this so + 1 for
        // correct indexing in the final vector

        let sub_imports = ModuleFinder::new(
            &metadata.src_bytes,
            metadata.script_start,
            metadata.serial_start,
        )
        .collect_imports(interner);

        let sub_mod = Module::new(name_id, *path_id, sub_imports, metadata);

        resolve_modules(seen, modules, &sub_mod.imports, mod_map, interner)?;

        mod_map.insert(name_id, ModuleId::new((modules.len() + 1) as u32));
        modules.push(sub_mod);
    }

    Ok(())
}
