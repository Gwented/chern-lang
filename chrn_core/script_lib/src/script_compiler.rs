use std::collections::HashMap;

use chrn_utils::{
    id_types::{InternedId, ModuleId, PathId, ScopeId, SymbolId, TypeId, ValueId},
    intern,
    types::{
        builtins::BuiltinType,
        type_constraints::{TypeConstraint, TypeConstraintFlags},
    },
    values::ValueInfo,
};

use crate::{
    modules::{Bind, Import, ImportKind, Module},
    semantic::{
        constraints::ArgConstraint,
        representation::{
            AliasDef, EnumDef, FuncDef, FuncKind, Param, ResolvedExpr, StructDef, Symbol,
            SymbolKind, Table, Type, TypeDef, TypeInfo,
        },
        scopes::{AssociatedScopeKind, IntrinsicRegistry, Scope, ScopeInfo, ScopeType},
    },
};

//TODO: A type state control flow where the TypeState is stored inside of types so that all types
//have a stable handle, whether or not their handle exists yet, so that variables can have their
//type id slot mutated into whatever type it needs so that static accessibility can assign it's
//expression a type handle even if the type of the end of the variable in the namespace hasn't been
//resolved yet.
//
// So in the case of: "module::NUMBER" where NUMBER is a valid static namespace, but hasn't been
// resolved yet, it'll store the possibly, "TypeState::Unresolved" index, which is notified later
// on.
/// "chrn" script compiler that holds all essential data for incremental updates through resolution
pub struct ScriptCompiler {
    /// Optional bind statement that is obtained from the main module
    // Maybe the module should keep it's bind info rather than give it to the compiler so that the
    // information isn't lossy and contextual
    pub bind: Option<Bind>,
    /// Module name to module id mapping to index module array. import `as` aliases are also stored here
    // This feels out of place
    // Can this be removed? Probably.
    // pub mod_map: HashMap<PathId, ModuleId>,
    /// All modules found during compilation
    pub mods: Vec<Module>,
    /// Type table which contains every module's stored types
    pub types: Vec<TypeInfo>,
    /// All values that were cached
    pub values: Vec<ValueInfo>,
    /// All expressions that were found
    pub exprs: Vec<ResolvedExpr>,
    /// All symbols that were found
    pub symbols: Vec<Symbol>,
    /// Scope arena
    pub scopes: Vec<ScopeInfo>,
    // em-dash
    // em-dash
    /// Information regarding intrinsic data such as core's `ModuleId`
    pub intrinsic_registry: IntrinsicRegistry,
}

//NOTE: I think these can be removed. Maybe.
pub const CORE_I8: u32 = 0;
pub const CORE_U8: u32 = 1;
pub const CORE_I16: u32 = 2;
pub const CORE_U16: u32 = 3;
pub const CORE_F16: u32 = 4;
pub const CORE_I32: u32 = 5;
pub const CORE_U32: u32 = 6;
pub const CORE_F32: u32 = 7;
pub const CORE_I64: u32 = 8;
pub const CORE_U64: u32 = 9;
pub const CORE_F64: u32 = 10;
pub const CORE_I128: u32 = 11;
pub const CORE_U128: u32 = 12;
pub const CORE_F128: u32 = 13;
pub const CORE_SIZED: u32 = 14;
pub const CORE_UNSIZED: u32 = 15;
pub const CORE_STR: u32 = 16;
pub const CORE_CHAR: u32 = 17;
pub const CORE_NIL: u32 = 18;
pub const CORE_BOOL: u32 = 19;
pub const CORE_BIGINT: u32 = 20;
pub const CORE_BIGFLOAT: u32 = 21;
pub const CORE_RUNTIME: u32 = 22;
pub const CORE_UNKNOWN: u32 = 23;
// pub const CORE_CHARACTER_MAPPABLE: u32 = 24;
// pub const CORE_LIST: u32 = 23;
// pub const CORE_SET: u32 = 24;
// pub const CORE_MAP: u32 = 25;
// pub const CORE_TUPLE: u32 = 26;
// Called idx but is u32...

// ----
// NOTE: May turn this into an innate option type inside of HIR
// Ok now this really needs to be an option
pub const VALUE_UNKNOWN: usize = 0;

impl ScriptCompiler {
    //FIX: Arbitrary ordering of pushes tied to the actual order of the enums. Should not be tied
    //to anything, similar to the interner's constants.
    /// Loads core library and builds script specific compiler with parameters given
    pub fn new(
        bind: Option<Bind>,
        // mod_map: HashMap<PathId, ModuleId>,
        mods: Vec<Module>,
    ) -> ScriptCompiler {
        //TEST:
        let core_mod_id = ModuleId::new(mods.len());
        let intrinsic_registry = IntrinsicRegistry::new(core_mod_id, None, None);
        let mut script_compiler = ScriptCompiler {
            bind,
            // mod_map,
            mods,
            types: Vec::new(),
            values: Vec::new(),
            exprs: Vec::new(),
            symbols: Vec::new(),
            scopes: Vec::new(),
            //TEST:
            intrinsic_registry,
        };

        // Should this lazy load the section intrinsics though?
        Self::load_core(&mut script_compiler);
        Self::create_module_symbols(&mut script_compiler);

        script_compiler
    }

    /// Creates the symbols needed for modules to be able to access to access their imports
    fn create_module_symbols(compiler: &mut ScriptCompiler) {
        // Loops through all modules, registering themselves as a symbol to themselves, iterating
        // through their imports to then inject those symbols as modules that can be looked up

        // So, if we have main AND other
        // It registers "main" as a module symbol so usage such as "main.MainType" can be used
        // It then registers a symbol for "other" so that the same "other.OtherType" semantics can
        // be done
        // If there is an alias, that is also ensured to be pushed as a symbol connected to the
        // module "other"
        for i in 0..compiler.mods.len() {
            let module = &compiler.mods[i];

            // Avoiding borrow issues by just storing the ids earlier
            let current_mod_name_id = module.name_id;
            let current_mod_id = module.mod_id;

            // Pushing the module symbol inside of itself. So if we're indexing module `main`, we
            // would be pushing `main` inside of itself, once, as a known symbol.
            let sym_id = SymbolId::new(compiler.symbols.len() as u32);
            let symbol = Symbol::new(
                current_mod_name_id,
                sym_id,
                None,
                current_mod_id,
                true,
                Some(AssociatedScopeKind::Module(current_mod_id)),
                ScopeType::Neutral,
                SymbolKind::Module(current_mod_id),
            );

            // Module symbols go into the neutral scope because, uh
            // Um
            let scope_id = compiler.push_scope(ScopeType::Neutral, current_mod_id);
            let scope = &mut compiler.get_scope_mut(scope_id).scope;
            scope
                .table
                .interned_to_sym
                .insert(current_mod_name_id, sym_id);
            compiler.symbols.push(symbol);

            // Re-borrowing for iteration
            let module = &compiler.mods[i];

            // Clone..
            for import in module.imports.clone() {
                let import_sym_id = SymbolId::new(compiler.symbols.len() as u32);
                // Pushing any imports found within the given module
                let symbol = Symbol::new(
                    import.name_id,
                    import_sym_id,
                    None,
                    current_mod_id,
                    true,
                    Some(AssociatedScopeKind::Module(import.mod_id)),
                    ScopeType::Neutral,
                    SymbolKind::Module(import.mod_id),
                );

                // Module symbols go into the neutral scope because, uh
                // Um
                let scope_id = compiler.push_scope(ScopeType::Neutral, current_mod_id);

                let scope = &mut compiler.get_scope_mut(scope_id).scope;
                scope
                    .table
                    .interned_to_sym
                    .insert(import.name_id, import_sym_id);
                compiler.symbols.push(symbol);

                // Maybe it can just point to the import directly instead of needing it's own
                // symbol?
                if let Some(alias_name_id) = import.alias_id {
                    let alias_sym_id = SymbolId::new(compiler.symbols.len() as u32);

                    // Pushing the alias associated with the import symbol if present
                    let symbol = Symbol::new(
                        alias_name_id,
                        alias_sym_id,
                        None,
                        current_mod_id,
                        true,
                        Some(AssociatedScopeKind::Module(import.mod_id)),
                        ScopeType::Neutral,
                        SymbolKind::Module(import.mod_id),
                    );

                    let scope = &mut compiler.get_scope_mut(scope_id).scope;
                    scope
                        .table
                        .interned_to_sym
                        .insert(alias_name_id, alias_sym_id);

                    compiler.symbols.push(symbol);
                }
            }
        }
    }

    pub(super) fn get_typedef(&self, sym_id: SymbolId) -> &TypeDef {
        match &self.symbols[sym_id.id as usize] {
            sym_info => match &sym_info.kind {
                SymbolKind::Type(type_id) => match &self.types[type_id.id as usize].ty {
                    Type::TypeDef(type_def) => type_def,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_typedef_mut(&mut self, sym_id: SymbolId) -> &mut TypeDef {
        match &self.symbols.get_mut(sym_id.id as usize).expect("misusage") {
            sym_info => match &sym_info.kind {
                SymbolKind::Type(type_id) => match &mut self.types[type_id.id as usize].ty {
                    Type::TypeDef(type_def) => type_def,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_struct(&self, sym_id: SymbolId) -> &StructDef {
        match &self.symbols[sym_id.id as usize] {
            sym_info => match &sym_info.kind {
                SymbolKind::Type(type_id) => match &self.types[type_id.id as usize].ty {
                    Type::Struct(struct_def) => struct_def,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_struct_mut(&mut self, sym_id: SymbolId) -> &mut StructDef {
        match self.symbols.get_mut(sym_id.id as usize).expect("misusage") {
            sym_info => match &mut sym_info.kind {
                SymbolKind::Type(type_id) => match &mut self.types[type_id.id as usize].ty {
                    Type::Struct(struct_def) => struct_def,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_func(&self, sym_id: SymbolId) -> &FuncDef {
        match &self.symbols[sym_id.id as usize] {
            sym_info => match &sym_info.kind {
                SymbolKind::Type(type_id) => match &self.types[type_id.id as usize].ty {
                    Type::Func(func_def) => func_def,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_func_mut(&mut self, sym_id: SymbolId) -> &mut FuncDef {
        match self.symbols.get_mut(sym_id.id as usize).expect("misusage") {
            sym_info => match &mut sym_info.kind {
                SymbolKind::Type(type_id) => match &mut self.types[type_id.id as usize].ty {
                    Type::Func(func_def) => func_def,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_enum(&self, sym_id: SymbolId) -> &EnumDef {
        match &self.symbols[sym_id.id as usize] {
            sym_info => match &sym_info.kind {
                SymbolKind::Type(type_id) => match &self.types[type_id.id as usize].ty {
                    Type::Enum(enum_def) => enum_def,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_enum_mut(&mut self, sym_id: SymbolId) -> &mut EnumDef {
        match self.symbols.get_mut(sym_id.id as usize).expect("misusage") {
            sym_info => match &mut sym_info.kind {
                SymbolKind::Type(type_id) => match &mut self.types[type_id.id as usize].ty {
                    Type::Enum(enum_def) => enum_def,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_alias(&self, sym_id: SymbolId) -> &AliasDef {
        match &self.symbols[sym_id.id as usize] {
            sym_info => match &sym_info.kind {
                SymbolKind::Type(type_id) => match &self.types[type_id.id as usize].ty {
                    Type::Alias(alias_def) => alias_def,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_alias_mut(&mut self, sym_id: SymbolId) -> &mut AliasDef {
        match self.symbols.get_mut(sym_id.id as usize).expect("Misusage") {
            sym_info => match &mut sym_info.kind {
                SymbolKind::Type(type_id) => match &mut self.types[type_id.id as usize].ty {
                    Type::Alias(alias_def) => alias_def,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    /// Assumes the symbol given is a variable, meaning a symbol with a value inside of it
    pub(super) fn get_var(&self, sym_id: SymbolId) -> &ValueInfo {
        match &self.symbols[sym_id.id as usize] {
            sym_info => match &sym_info.kind {
                SymbolKind::Val(val_id) => &self.values[val_id.id as usize],
                _ => unreachable!(),
            },
        }
    }

    /// Assumes the symbol given is a variable, meaning a symbol with a value inside of it
    pub(super) fn get_var_mut(&mut self, sym_id: SymbolId) -> &mut ValueInfo {
        match &self.symbols[sym_id.id as usize] {
            sym_info => match &sym_info.kind {
                SymbolKind::Val(val_id) => &mut self.values[val_id.id as usize],
                _ => unreachable!(),
            },
        }
    }

    // Maybe return option?
    /// Assumes the symbol given has a `TypeId` attached. Will return a `TypeId` of `Unknown` if
    /// the `SymbolKind` is unknown.
    pub(super) fn get_type_id(&self, sym_id: SymbolId) -> TypeId {
        match &self.symbols[sym_id.id as usize] {
            sym_info => match &sym_info.kind {
                SymbolKind::Type(type_id) => *type_id,
                SymbolKind::Val(val_id) => self.values[val_id.id as usize].type_id,
                SymbolKind::ReservedTypeSlot(type_id) => *type_id,
                SymbolKind::Module(_) => unreachable!(),
            },
        }
    }

    /// Returns `ModuleId` which is the module of origin
    pub fn get_owner(&self, sym_id: SymbolId) -> ModuleId {
        self.symbols[sym_id.id as usize].owner
    }

    /// Get's the `ScopeId` with no assumption of it existing.
    ///
    /// This method exists along with extract_scope_id due to cross module namespace checking not
    /// innately confirming whether or not it contains a particular `ScopeType`
    //FIX: Id for consistency
    pub fn get_scope_id(&self, scope_type: ScopeType, owner: ModuleId) -> Option<ScopeId> {
        self.find_scope(scope_type, owner).map(|s| s.scope.scope_id)
    }

    /// Get's the `ScopeId` assuming that the scope already exists. Panics otherwise.
    ///
    /// This exists because if the current module has something like a typedef in the semantic stage,
    /// that means the parser itself already checked if it was legal grammar-wise.
    pub fn extract_scope_id(&self, scope_type: ScopeType, owner_id: ModuleId) -> ScopeId {
        self.find_scope(scope_type, owner_id)
            .expect("Either misuage of function, semantic broke, parser broke, or modules broke")
            .scope
            .scope_id
    }

    /// Get's scope using a `ScopeId`
    pub fn get_scope(&self, scope_id: ScopeId) -> &ScopeInfo {
        &self.scopes[scope_id.id]
    }

    /// Returns mutably borrowed `ScopeInfo` using a `ScopeId`
    pub fn get_scope_mut(&mut self, scope_id: ScopeId) -> &mut ScopeInfo {
        &mut self.scopes[scope_id.id]
    }

    /// Pushes new scope with given scope type and returns the `ScopeId`. If the scope already
    /// exists then it returns the existent `ScopeId`.
    pub fn push_scope(&mut self, scope_type: ScopeType, owner_id: ModuleId) -> ScopeId {
        if let Some(scope_info) = self.find_scope(scope_type, owner_id) {
            return scope_info.scope.scope_id;
        }

        let scope_id = ScopeId::new(self.scopes.len());
        // Beep
        let intrinsic_scope_opt: Option<ScopeId> = match scope_type {
            ScopeType::Complex => {
                if let Some(scope_id) = self.intrinsic_registry.complex {
                    Some(scope_id)
                } else {
                    let scope_id = self.load_complex_constants();
                    Some(scope_id)
                }
            }
            ScopeType::Override => {
                if let Some(scope_id) = self.intrinsic_registry.overrid {
                    Some(scope_id)
                } else {
                    let scope_id = self.load_override_constants();
                    Some(scope_id)
                }
            }
            ScopeType::Local
            | ScopeType::Neutral
            | ScopeType::Var
            | ScopeType::Nest
            | ScopeType::Core => None,
        };

        let scope = Scope::new(scope_id, scope_type, false, intrinsic_scope_opt);
        let scope_info = ScopeInfo::new(scope, None, owner_id);
        self.scopes.push(scope_info);

        let owner_mod = &mut self.mods[owner_id.id];
        owner_mod.scopes.push(scope_id);
        // owner_mod.held_scopes |= scope_type.to_u8();
        // BRAIN OFF

        scope_id
    }

    /// Returns `Some` scope under the given kind if it exists, `None` otherwise.
    //NOTE: May opt for indices similarly to the ast's way of making sections
    pub fn find_scope(&self, scope_type: ScopeType, owner_id: ModuleId) -> Option<&ScopeInfo> {
        let mod_owner = &self.mods[owner_id.id];
        for scope_id in &mod_owner.scopes {
            let scope_info = &self.scopes[scope_id.id];
            if scope_info.scope.scope_type == scope_type {
                return Some(scope_info);
            }
        }

        None
    }

    // //TEST:
    // pub fn find_scope(
    //     &self,
    //     scope_type: ScopeType,
    //     associated_scope: AssociatedScopeKind,
    // ) -> Option<&ScopeInfo> {
    //     match associated_scope {
    //         AssociatedScopeKind::Module(mod_id) => {
    //             let mod_owner = &self.mods[mod_id.id];
    //
    //             for scope_id in &mod_owner.scopes {
    //                 let scope_info = &self.scopes[scope_id.id];
    //                 if scope_info.scope.scope_type == scope_type {
    //                     return Some(scope_info);
    //                 }
    //             }
    //         }
    //         // Seems a little odd
    //         // Probably shouldn't account for it's type here
    //         AssociatedScopeKind::Scope(scope_id) => {
    //             let scope_info = &self.scopes[scope_id.id];
    //             if scope_info.scope.scope_type == scope_type {
    //                 return Some(scope_info);
    //             }
    //         }
    //     }
    //
    //     None
    // }

    /// Loads the core module
    fn load_core(compiler: &mut ScriptCompiler) {
        let mut table = Table::new();

        //TODO: If namespace core exists as a module then should error earlier
        let core_name_id = InternedId::new(intern::INTERNED_CORE);
        let core_mod_id = ModuleId::new(compiler.mods.len());
        let core_scope_id = ScopeId::new(compiler.scopes.len());
        let mut core_mod = Module::new(core_name_id, core_mod_id, Vec::new(), None);

        Self::load_core_types(compiler, &core_mod, &mut table);
        Self::load_core_funcs(compiler, &core_mod, &mut table);
        // Self::load_complex_constants(compiler, &mut core_mod, &mut table);
        // Self::load_override_constants(compiler, &mut core_mod, &mut table);

        // Exporting all created symbols from core
        for sym_id in table.interned_to_sym.values().copied() {
            core_mod.exports.push(sym_id);
        }

        // Done adding all of core
        let scope_id = ScopeId::new(compiler.scopes.len());
        let scope = Scope::with_table(scope_id, ScopeType::Core, None, true, table);
        let scope_info = ScopeInfo::new(scope, None, core_mod_id);

        compiler.scopes.push(scope_info);
        core_mod.scopes.push(scope_id);

        compiler.mods.push(core_mod);

        let core_import = Import::new(core_name_id, core_mod_id, ImportKind::Core, None);

        // Injecting core as an import and pushing it's scope so user modules can search it
        for user_mod in &mut compiler.mods {
            if user_mod.name_id == core_name_id {
                continue;
            }

            user_mod.imports.push(core_import.clone());
            user_mod.scopes.push(core_scope_id);
        }
    }

    /// Returns `true` if the type is unknown, false otherwise
    pub fn check_unknown(&self, type_id: TypeId) -> bool {
        let ty = &self.types[type_id.id as usize].ty;
        match ty {
            // Can't do this since a type, may depend on a type, that pointer to another type.
            // May change this system since it is concerning to rely on such possibly deep
            // recursive issues that could be hidden.
            // Type::Deferred(deferred_type_id) => {
            //     let deferred_ty = &self.types[deferred_type_id.id as usize].ty;
            //     match deferred_ty {
            //         Type::Unknown => true,
            //         // Type::Deferred(_) => {
            //         //     panic!("Encountered infinitely recursive deferred type in `check_unknown`")
            //         // }
            //         _ => false,
            //     }
            // }
            //WARN: DANGEROUS.
            Type::Deferred(type_id) => self.check_unknown(*type_id),
            Type::Unknown => true,
            _ => false,
        }
    }

    //TODO: There is an issue with how scopes are consumed right now which makes giving specific
    //scopes known constants difficult. Since there is no one source of data for a section to get
    //it's constants, it isn't possible to make it so if we are in a `complex->` section, it shows
    //language specific constants like RUST or JAVA which all for specifying behavior. All
    //scopes are locally owned and don't separate what declared can be used for all other scopes,
    //and which are just local.
    //
    // There should be more percise access level rules to where a variable declaration will allow
    // all other scopes to use it, but also have where it's declaration occurred be tied to it,
    // while also allowing for a section like `complex` to show the `RUST` constant only in it's
    // own scope.
    //
    // This would probably require pre-loading section symbols on-demand to where their
    // associated_scope is immediately attached to all the resolver stages. So maybe a
    // ScopeType::Global is needed.
    //
    // First lets focus on how pre-loading would work
    //
    // Ok what about, if not found in normal scope, search intrinsic, where now scopes carry
    // Option<ScopeId's> which allow for their intrinsics to be searched
    /// Creates scope with the constants needed for a `complex` section to function then returns
    /// it's `ScopeId`
    fn load_complex_constants(&mut self) -> ScopeId {
        // IS it from core? The semantics are getting a little lost
        let core_mod_id = self.intrinsic_registry.core_mod_id;
        let scope_type = ScopeType::Complex;
        let complex_scope_id = ScopeId::new(self.scopes.len());

        let mut table = Table::new();

        todo!()
    }

    /// Creates scope with the constants needed for an `override` section to function then returns
    /// it's `ScopeId`
    fn load_override_constants(&mut self) -> ScopeId {
        // IS it from core? The semantics are getting a little lost
        let core_mod_id = self.intrinsic_registry.core_mod_id;
        let scope_type = ScopeType::Override;
        let override_scope_id = ScopeId::new(self.scopes.len());

        let mut table = Table::new();
        self.load_override_java_symbols(&mut table, override_scope_id, core_mod_id);

        let scope = Scope::with_table(override_scope_id, scope_type, None, true, table);
        let java_scope = ScopeInfo::new(scope, None, core_mod_id);
        todo!()
    }

    fn load_override_java_symbols(
        &mut self,
        table: &mut Table,
        complex_scope_id: ScopeId,
        core_mod_id: ModuleId,
    ) {
        let name_id = InternedId::new(intern::INTERNED_JAVA_UPPER);
        let sym_id = SymbolId::new(self.symbols.len() as u32);
        let java_symbol = Symbol::new(
            name_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Complex,
            SymbolKind::Module(todo!()),
        );

        todo!()
    }

    /// Helper to load all of core's functions and predicates
    fn load_core_funcs(compiler: &mut ScriptCompiler, core_mod: &Module, table: &mut Table) {
        let core_mod_id = core_mod.mod_id;

        // IsEmpty
        let type_id = TypeId::new(compiler.types.len() as u32);
        let is_empty_flags = TypeConstraintFlags::new(TypeConstraint::Collection.to_u64());

        let func_def = FuncDef::new(
            FuncKind::IsEmpty,
            false,
            is_empty_flags,
            vec![ArgConstraint::ArgCount(0)],
            true,
            TypeId::new(CORE_BOOL),
        );
        compiler
            .types
            .push(TypeInfo::new(Type::Func(func_def), core_mod_id));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_IS_EMPTY);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        // IsWhitespace | CharacterMappable
        let type_id = TypeId::new(compiler.types.len() as u32);
        let ws_flags = TypeConstraintFlags::new(TypeConstraint::CharacterMappable.to_u64());

        let func_def = FuncDef::new(
            FuncKind::IsWhitespace,
            false,
            ws_flags,
            vec![ArgConstraint::ArgCount(0), ArgConstraint::CharacterMappable],
            true,
            TypeId::new(CORE_BOOL),
        );

        compiler
            .types
            .push(TypeInfo::new(Type::Func(func_def), core_mod_id));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_IS_WHITESPACE);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        // Contains(String | char) CharacterMappable
        let type_id = TypeId::new(compiler.types.len() as u32);
        let contains_flags = TypeConstraintFlags::new(TypeConstraint::CharacterMappable.to_u64());

        let func_def = FuncDef::new(
            FuncKind::Contains,
            true,
            contains_flags,
            vec![ArgConstraint::ArgCount(1), ArgConstraint::CharacterMappable],
            true,
            TypeId::new(CORE_BOOL),
        );

        compiler
            .types
            .push(TypeInfo::new(Type::Func(func_def), core_mod_id));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_CONTAINS);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        // StartsW(Value) | CharacterMappable
        let type_id = TypeId::new(compiler.types.len() as u32);
        let startsw_flags = TypeConstraintFlags::new(TypeConstraint::CharacterMappable.to_u64());

        let func_def = FuncDef::new(
            FuncKind::StartsW,
            true,
            startsw_flags,
            vec![ArgConstraint::ArgCount(1), ArgConstraint::CharacterMappable],
            true,
            TypeId::new(CORE_BOOL),
        );

        compiler
            .types
            .push(TypeInfo::new(Type::Func(func_def), core_mod_id));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_STARTSW);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        // EndsW(Value) | CharacterMappable
        let type_id = TypeId::new(compiler.types.len() as u32);
        let endsw_flags = TypeConstraintFlags::new(TypeConstraint::CharacterMappable.to_u64());

        let func_def = FuncDef::new(
            FuncKind::EndsW,
            true,
            // What about CharacterMappable? Do we really want to be judgemental here?
            // There we go
            endsw_flags,
            vec![ArgConstraint::ArgCount(1), ArgConstraint::CharacterMappable],
            true,
            TypeId::new(CORE_BOOL),
        );

        compiler
            .types
            .push(TypeInfo::new(Type::Func(func_def), core_mod_id));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_ENDSW);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        // Range(inclusive, inclusive) | Numeric | Ordering
        let type_id = TypeId::new(compiler.types.len() as u32);
        let range_flags = TypeConstraintFlags::new(TypeConstraint::Ranged.to_u64());
        let func_def = FuncDef::new(
            FuncKind::Range,
            true,
            range_flags,
            vec![
                ArgConstraint::ArgCount(2),
                ArgConstraint::Numeric,
                ArgConstraint::MatchingArgumentTypes,
                ArgConstraint::SameTypeAsSelf,
            ],
            true,
            TypeId::new(CORE_BOOL),
        );

        compiler
            .types
            .push(TypeInfo::new(Type::Func(func_def), core_mod_id));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_RANGE);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        // Equals(Comparable)
        let type_id = TypeId::new(compiler.types.len() as u32);
        let eq_flags = TypeConstraintFlags::new(TypeConstraint::Comparable.to_u64());

        let func_def = FuncDef::new(
            FuncKind::Equals,
            true,
            eq_flags,
            vec![
                ArgConstraint::ArgCount(1),
                ArgConstraint::Comparable,
                ArgConstraint::SameTypeAsSelf,
            ],
            true,
            TypeId::new(CORE_BOOL),
        );

        compiler
            .types
            .push(TypeInfo::new(Type::Func(func_def), core_mod_id));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_EQUALS);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);
    }

    // --- Beep
    /// Helper to load all of core's types
    fn load_core_types(compiler: &mut ScriptCompiler, core_mod: &Module, table: &mut Table) {
        let core_mod_id = core_mod.mod_id;

        // -- Concrete types --

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::I8),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_I8);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::U8),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_U8);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::I16),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_I16);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::U16),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_U16);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::F16),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_F16);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::I32),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_I32);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::U32),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_U32);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::F32),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_F32);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::I64),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_I64);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::U64),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_U64);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::F64),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_F64);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::I128),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_I128);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::U128),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_U128);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::F128),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_F128);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::Sized),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_SIZED);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::Unsized),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_UNSIZED);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::Str),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_STR);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::Char),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_CHAR);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::Nil),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_NIL);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::Bool),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_BOOL);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::BigInt),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_BIGINT);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::BigFloat),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_BIGFLOAT);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );
        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::Runtime),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_RUNTIME);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        compiler
            .types
            .push(TypeInfo::new(Type::Unknown, core_mod_id));

        // -- Type constraints --
        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Constrained(TypeConstraintFlags::new(TypeConstraint::Ranged.to_u64())),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_RANGED);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Constrained(TypeConstraintFlags::new(
                TypeConstraint::CharacterMappable.to_u64(),
            )),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_CHARACTER_MAPPABLE);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Constrained(TypeConstraintFlags::new(
                TypeConstraint::Collection.to_u64(),
            )),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_COLLECTION);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Constrained(TypeConstraintFlags::new(TypeConstraint::HasLen.to_u64())),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_HAS_LEN);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Constrained(TypeConstraintFlags::new(TypeConstraint::Integer.to_u64())),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_INTEGER);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        // Numeric
        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Constrained(TypeConstraintFlags::new(TypeConstraint::Numeric.to_u64())),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_NUMERIC);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Constrained(TypeConstraintFlags::new(
                TypeConstraint::SignedInteger.to_u64(),
            )),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_SIGNED_INTEGER);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Constrained(TypeConstraintFlags::new(
                TypeConstraint::UnsignedInteger.to_u64(),
            )),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_UNSIGNED_INTEGER);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Constrained(TypeConstraintFlags::new(TypeConstraint::Float.to_u64())),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_FLOAT);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Constrained(TypeConstraintFlags::new(TypeConstraint::Ordered.to_u64())),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_ORDERED);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        let type_id = TypeId::new(compiler.types.len() as u32);
        compiler.types.push(TypeInfo::new(
            Type::Constrained(TypeConstraintFlags::new(
                TypeConstraint::Comparable.to_u64(),
            )),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_COMPARABLE);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            None,
            ScopeType::Core,
            SymbolKind::Type(type_id),
        );

        compiler.symbols.push(symbol);
        table.interned_to_sym.insert(interned_id, sym_id);
    }
}
