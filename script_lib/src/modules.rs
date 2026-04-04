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
    pub bind: Option<NameId>,
    pub mods: Vec<Module>,
}

impl Program {
    pub fn new(bind: Option<NameId>) -> Program {
        Program {
            bind,
            mods: Vec::new(),
        }
    }
}

// What about OUR name?
#[derive(Debug)]
pub struct Module {
    pub path_id: PathId,
    pub imports: Vec<PathId>,
    pub metadata: ChernMetadata,
    pub table: Table,
}

impl Module {
    pub fn new(path_id: PathId, imports: Vec<PathId>, metadata: ChernMetadata) -> Module {
        Module {
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

    let path_id = PathId::new(interner.intern_path(path));

    let main_imports = ModuleFinder::new(
        &main_metadata.src_bytes,
        main_metadata.script_start,
        main_metadata.serial_start,
    )
    .collect_imports(interner);

    let main_mod = Module::new(path_id, main_imports, main_metadata);

    let mut seen: HashSet<PathId> = HashSet::new();
    seen.insert(main_mod.path_id);

    // Will incur borrowing issues unless the main_mod is put in last since the list of it's
    // imports is needed to start recursive process
    let mut other_mods: Vec<Module> = Vec::new();
    resolve_modules(&mut seen, &mut other_mods, &main_mod.imports, interner)?;

    // Will change
    let mut all_mods: Vec<Module> = Vec::new();
    all_mods.push(main_mod);
    all_mods.append(&mut other_mods);

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

/// This function takes recursively resolves each import within a hierarchy of imports, after being
/// given a root main import to go off of.
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

        let sub_imports = ModuleFinder::new(
            &metadata.src_bytes,
            metadata.script_start,
            metadata.serial_start,
        )
        .collect_imports(interner);

        let sub_mod = Module::new(*path_id, sub_imports, metadata);

        resolve_modules(seen, modules, &sub_mod.imports, interner).unwrap();

        modules.push(sub_mod);
    }

    Ok(())
}
