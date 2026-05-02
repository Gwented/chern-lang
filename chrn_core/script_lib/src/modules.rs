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
    semantic::scopes::{Scope, ScopeType},
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
#[derive(Debug)]
pub struct Module {
    /// File name that will be used internally
    pub name_id: InternedId,
    /// Actual path used to find the file itself
    pub path_id: PathId,
    /// It's own module id position
    pub mod_id: ModuleId,
    /// Imports found in the module
    pub imports: Vec<Import>,
    /// Represents the 4 existent scopes
    pub scopes: Vec<Scope>,
    pub metadata: ModuleMetadata,
}

impl Module {
    pub fn new(
        name_id: InternedId,
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
            scopes: Vec::new(),
            metadata,
        }
    }

    /// Get's the `ScopeId` with no assumption of it existing.
    ///
    /// This method exists along with extract_scope_id due to cross module namespace checking not
    /// innately confirming whether or not it contains a particular `ScopeType`
    pub fn get_scope_id(&self, scope_type: ScopeType) -> Option<ScopeId> {
        self.find_scope(scope_type).map(|s| s.scope_id)
    }

    /// Get's the `ScopeId` assuming that the scope already exists. Panics otherwise.
    ///
    /// This exists because if the current module has something like a typedef in the semantic stage,
    /// that means the parser itself already checked if it was legal grammar-wise.
    pub fn extract_scope_id(&self, scope_type: ScopeType) -> ScopeId {
        self.find_scope(scope_type)
            .expect("Either semantic broke, parser broke, or modules broke")
            .scope_id
    }

    /// Get's scope using a `ScopeId`
    pub fn get_scope(&self, scope_id: ScopeId) -> &Scope {
        &self.scopes[scope_id.id]
    }

    /// Get's mutably borrowed scope using a `ScopeId`
    pub fn get_scope_mut(&mut self, scope_id: ScopeId) -> &mut Scope {
        &mut self.scopes[scope_id.id]
    }

    /// Pushes new scope with given scope type and returns the `ScopeId`. If the scope already
    /// exists then it returns the existent `ScopeId`.
    pub fn push_scope(&mut self, scope_type: ScopeType) -> ScopeId {
        if let Some(scope) = self.find_scope(scope_type) {
            return scope.scope_id;
        }

        let scope_id = ScopeId::new(self.scopes.len());
        self.scopes.push(Scope::new(scope_id, scope_type));

        scope_id
    }

    /// Checks if the name id corresponds to a `SymbolId` within the given `ScopeType`.
    /// Returns a tuple of the `AstId` and `ScopeType` the `NameId` was found in. Returns None if
    /// no accessible scopes contain the given `NameId`.
    pub fn get_sym_id(&self, name_id: InternedId, scope_type: ScopeType) -> Option<SymbolId> {
        // I don't think this can fail. Should maybe expect for clarity.
        let allowed_scope_types = scope_type.accessible_scopes();

        // Loops over all allowed scopes and checks their individual namespaces
        for allowed_scope_type in allowed_scope_types {
            // In this scenario the scope may or may not exist since this could be used from
            // another module
            if let Some(scope) = self.find_scope(allowed_scope_type) {
                for (current_ast_id, current_name_id) in &scope.table.name_ids {
                    if *current_name_id == name_id {
                        let scope_id = self.extract_scope_id(allowed_scope_type);
                        let scope = self.get_scope(scope_id);

                        let sym_id = scope.table.sym_ids[&current_ast_id];
                        return Some(sym_id);
                    }
                }
            }
        }

        None
    }

    /// Returns Some scope if it exists, None otherwise
    fn find_scope(&self, scope_type: ScopeType) -> Option<&Scope> {
        for scope in &self.scopes {
            if scope.scope_type == scope_type {
                return Some(scope);
            }
        }

        None
    }
}

#[derive(Debug)]
pub struct ModuleMetadata {
    /// Bytes from chrn config file
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
/// Takes in a path to a `chrn` config file, then recursively resolved all imports associated with
/// the path given in separate modules.
pub fn extract_modules(
    // Does this get canonicalized here or earlier..
    path: &Path,
    settings: &ChrnSettings,
    interner: &mut Intern,
) -> Result<ScriptCompiler, ConfigLoadError> {
    // This MUST be explicitly

    let src = match file_ops::fopen(&path) {
        Ok(f) => f,
        Err(err_msg) => {
            // This is the sole reason the span is an option
            let diag = Diagnostic::new(path, err_msg.clone(), None, err_msg, Area::ConfigLoad);
            return Err(ConfigLoadError::Module(diag));
        }
    };

    let path = path.canonicalize()?;

    let main_metadata = ChrnConfigLoader::new(&path, src, settings).load_config()?;

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
    let path_id = PathId::new(interner.intern_path(&path));

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
    let main_mod = Module::new(name_id, path_id, mod_id, main_imports, main_metadata);

    let mut mod_map: HashMap<InternedId, ModuleId> = HashMap::new();
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
        interner,
    )?;

    // May change
    // Please change
    let mut all_mods: Vec<Module> = Vec::new();
    all_mods.push(main_mod);
    all_mods.append(&mut other_mods);

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
                    &prev_mod.metadata.src_bytes,
                    &[import.path_span],
                    settings.can_color,
                );

                let prev_path = interner.search_path(prev_mod.path_id.id as usize);
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
                    &prev_mod.metadata.src_bytes,
                    &[import.path_span],
                    settings.can_color,
                );

                let prev_path = interner.search_path(prev_mod.path_id.id as usize);
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

        let mod_metadata = ChrnConfigLoader::new(path, src, settings).load_config()?;

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

        let origin = interner.search_path(prev_mod.path_id.id as usize);

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
            import.path_id,
            ModuleId::new(current_mod_id),
            sub_imports,
            mod_metadata,
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
