use std::{
    collections::{HashMap, HashSet},
    fs,
    path::Path,
};

use common::{
    builtins::BuiltinType,
    config_loader::ChernConfigLoader,
    core_error::ConfigLoadError,
    intern::Intern,
    keywords,
    metadata::{ChernSettings, ModuleMetadata},
    reporter,
    symbols::{ModuleId, NameId, PathId, SymbolId},
};
pub mod mod_finder;

use crate::{
    ir::values::Value,
    iyo::file_ops,
    modules::mod_finder::ModuleFinder,
    parser::ast::{Bind, Import},
    semantic::representation::{
        ConstRepre, EnumRepre, FuncRepre, StructRepre, Symbol, SymbolInfo, Table, Type,
        TypeDefRepre, TypeInfo,
    },
};

pub struct Program {
    pub bind: Option<Bind>,
    pub mod_map: HashMap<NameId, ModuleId>,
    pub mods: Vec<Module>,
    pub types: Vec<TypeInfo>,
    pub values: Vec<Value>,
    pub(crate) symbols: HashMap<SymbolId, SymbolInfo>,
}

impl Program {
    pub fn new(
        bind: Option<Bind>,
        mod_map: HashMap<NameId, ModuleId>,
        mods: Vec<Module>,
    ) -> Program {
        let mut types: Vec<TypeInfo> = Vec::new();

        // Pre-loading keywords
        // If this fails something was messed up within keywords itself
        for i in 0..keywords::TYPE_END - 5 {
            let ty = BuiltinType::try_from_id(i as u32).expect("Builtin type not updated");
            types.push(TypeInfo::new(Type::BuiltinType(ty), None));
        }

        // Pre-loading Null
        let mut values: Vec<Value> = Vec::new();
        values.push(Value::Unknown);

        Program {
            bind,
            mod_map,
            mods,
            types,
            values,
            symbols: HashMap::new(),
        }
    }

    // Is there a reason to return err?
    pub(super) fn get_typedef(&self, sym_id: SymbolId) -> &TypeDefRepre {
        match &self.symbols[&sym_id] {
            sym_info => match &sym_info.symbol {
                Symbol::TypeDef(type_def_repre) => type_def_repre,
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_typedef_mut(&mut self, sym_id: SymbolId) -> &mut TypeDefRepre {
        match self.symbols.get_mut(&sym_id) {
            Some(sym_info) => match &mut sym_info.symbol {
                Symbol::TypeDef(type_def_repre) => type_def_repre,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }

    pub(super) fn get_struct(&self, sym_id: SymbolId) -> &StructRepre {
        match self.symbols.get(&sym_id) {
            Some(sym_info) => match &sym_info.symbol {
                Symbol::Struct(struct_repre) => struct_repre,
                _ => unreachable!(),
            },
            None => unreachable!(),
        }
    }

    pub(super) fn get_struct_mut(&mut self, sym_id: SymbolId) -> &mut StructRepre {
        match self.symbols.get_mut(&sym_id) {
            Some(sym_info) => match &mut sym_info.symbol {
                Symbol::Struct(struct_repre) => struct_repre,
                _ => unreachable!(),
            },
            None => unreachable!(),
        }
    }

    pub(super) fn get_func(&self, sym_id: SymbolId) -> &FuncRepre {
        match &self.symbols[&sym_id] {
            sym_info => match &sym_info.symbol {
                Symbol::Func(func_repre) => func_repre,
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_func_mut(&mut self, sym_id: SymbolId) -> &mut FuncRepre {
        match self.symbols.get_mut(&sym_id) {
            Some(sym_info) => match &mut sym_info.symbol {
                Symbol::Func(func_repre) => func_repre,
                _ => unreachable!(),
            },
            None => unreachable!(),
        }
    }

    pub(super) fn get_enum(&self, sym_id: SymbolId) -> &EnumRepre {
        match &self.symbols[&sym_id] {
            sym_info => match &sym_info.symbol {
                Symbol::Enum(enum_repre) => enum_repre,
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_enum_mut(&mut self, sym_id: SymbolId) -> &mut EnumRepre {
        match self.symbols.get_mut(&sym_id) {
            Some(sym_info) => match &mut sym_info.symbol {
                Symbol::Enum(enum_repre) => enum_repre,
                _ => unreachable!(),
            },
            None => unreachable!(),
        }
    }

    pub(super) fn get_const(&self, sym_id: SymbolId) -> &ConstRepre {
        match &self.symbols[&sym_id] {
            sym_info => match &sym_info.symbol {
                Symbol::Const(const_repre) => const_repre,
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_const_mut(&mut self, sym_id: SymbolId) -> &mut ConstRepre {
        match self.symbols.get_mut(&sym_id) {
            Some(sym_info) => match &mut sym_info.symbol {
                Symbol::Const(const_repre) => const_repre,
                _ => unreachable!(),
            },
            None => unreachable!(),
        }
    }

    pub(super) fn get_owner(&self, sym_id: SymbolId) -> ModuleId {
        self.symbols[&sym_id].owner
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
    /// It's own module id position
    pub mod_id: ModuleId,
    /// Imports found in the module
    pub imports: Vec<Import>,
    pub metadata: ModuleMetadata,
    pub table: Table,
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
