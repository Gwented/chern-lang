use std::{
    collections::HashSet,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

use common::{
    config_loader::ChernConfigLoader,
    core_error::{ConfigLoadError, ScriptError},
    intern::Intern,
    metadata::ChernMetadata,
    symbols::{NameId, PathId},
};
pub mod mod_finder;

use crate::{modules::mod_finder::ModuleFinder, semantic::representation::Table};

pub struct Program {
    // MOOOOOOOOOOOOOOODSSSS MODS
    pub bind: Option<PathId>,
    pub mods: Vec<Module>,
}

impl Program {
    pub fn new(bind: Option<PathId>) -> Program {
        Program {
            bind,
            mods: Vec::new(),
        }
    }
}

// What about OUR name?
#[derive(Debug)]
pub struct Module {
    /// File name that will be used internally
    pub file_id: NameId,
    /// Actual path id used to find the file itself
    pub path_id: PathId,
    // May need FileId, PathId
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
            file_id,
            path_id,
            imports,
            metadata,
            table: Table::new(),
        }
    }
}

//TEST: Lets depending on self recursively as a module happen for now
pub fn extract_modules(path: &Path, interner: &mut Intern) -> Result<Vec<Module>, ConfigLoadError> {
    //???????????????????????????????????????????
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

    let mut seen: HashSet<PathId> = HashSet::new();
    seen.insert(main_mod.path_id);

    // Will incur borrowing issues unless the main_mod is put in last since the list of it's
    // imports is needed to start recursive process
    let mut other_mods: Vec<Module> = Vec::new();
    resolve_modules(&mut seen, &mut other_mods, &main_mod.imports, interner)?;

    // May change
    let mut all_mods: Vec<Module> = Vec::new();
    all_mods.push(main_mod);
    all_mods.append(&mut other_mods);

    // Module viewing command
    for module in &all_mods {
        println!(
            "Module -> {}",
            interner.search_path(module.path_id.id as usize).display()
        );
        for path_id in &module.imports {
            println!(
                "\tImport -> {}",
                interner.search_path(path_id.id as usize).display()
            );
        }
        println!("_______\n")
    }

    Ok(all_mods)
}

/// This function recursively resolves each import within a hierarchy of imports after being
/// given a root import to go off of.
fn resolve_modules(
    seen: &mut HashSet<PathId>,
    modules: &mut Vec<Module>,
    imports: &Vec<PathId>,
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

        let file_id = NameId::new(interner.intern(&file_name));

        let sub_imports = ModuleFinder::new(
            &metadata.src_bytes,
            metadata.script_start,
            metadata.serial_start,
        )
        .collect_imports(interner);

        let sub_mod = Module::new(file_id, *path_id, sub_imports, metadata);

        resolve_modules(seen, modules, &sub_mod.imports, interner).unwrap();

        modules.push(sub_mod);
    }

    Ok(())
}
