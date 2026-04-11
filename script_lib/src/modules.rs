use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use common::{
    config_loader::ChernConfigLoader,
    core_error::ConfigLoadError,
    intern::Intern,
    metadata::ModuleMetadata,
    symbols::{ModuleId, NameId, PathId},
};
pub mod mod_finder;

use crate::{
    modules::mod_finder::ModuleFinder, parser::ast::Import, semantic::representation::Table,
};

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
pub fn extract_modules(path: &Path, interner: &mut Intern) -> Result<Program, ConfigLoadError> {
    // Maybe the cli should still do something about this since if the first path given
    // isn't valid, it WOULD warrent a basic error
    let src = fs::File::open(path)?;
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

    let main_imports = ModuleFinder::new(
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

    let program = Program::new(None, mod_map, all_mods);

    Ok(program)
}

/// This function recursively resolves each import after being given a root module with imports to go off of.
fn resolve_modules(
    seen: &mut HashSet<PathId>,
    modules: &mut Vec<Module>,
    imports: &Vec<Import>,
    mod_map: &mut HashMap<NameId, ModuleId>,
    interner: &mut Intern,
) -> Result<(), ConfigLoadError> {
    for import in imports {
        if seen.contains(&import.path_id) {
            continue;
        }

        seen.insert(import.path_id);

        let path = interner.search_path(import.path_id.id as usize);
        // TODO: Non-hacky way of getting spans
        let src = match fs::File::open(path) {
            Ok(f) => f,
            Err(e) => return Err(e.into()),
        };

        let mod_metadata = ChernConfigLoader::new(path, src).load_config()?;

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

        let sub_imports = ModuleFinder::new(
            &mod_metadata.src_bytes,
            mod_metadata.script_start,
            mod_metadata.serial_start,
        )
        .collect_imports(interner);

        let sub_mod = Module::new(name_id, import.path_id, sub_imports, mod_metadata);

        resolve_modules(seen, modules, &sub_mod.imports, mod_map, interner)?;

        // Modules start off at 0 since the main module can't be inserted before this so + 1 for
        // correct indexing in the final vector
        mod_map.insert(name_id, ModuleId::new((modules.len() + 1) as u32));
        modules.push(sub_mod);
    }

    Ok(())
}
