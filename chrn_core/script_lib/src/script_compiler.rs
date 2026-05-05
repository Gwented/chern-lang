use std::collections::HashMap;

use chrn_utils::{
    builtins::BuiltinType,
    id_types::{InternedId, ModuleId, ScopeId, SymbolId, TypeId},
    intern,
    values::ValueInfo,
};

use crate::{
    modules::{Bind, Module},
    semantic::{
        representation::{
            AliasDef, EnumDef, FuncDef, ResolvedExpr, StructDef, Symbol, SymbolKind, Table, Type,
            TypeDef, TypeInfo,
        },
        scopes::{Scope, ScopeInfo, ScopeType},
    },
};

//TODO: Intrinsic scope that holds value, type, etc. tables that is just a scope that is innate by
//default, which can be explicitly referenced. Like, intrinsic.str is just the default str
//identification, so no escape needed anymore for that
#[derive(Debug)]
pub struct ScriptCompiler {
    /// Optional bind statement that is obtained from the main module
    // Maybe the module should keep it's bind info rather than give it to the compiler so that the
    // information isn't lossy and contextual
    pub bind: Option<Bind>,
    /// Module name to module id mapping to index module array. import `as` aliases are also stored here
    pub mod_map: HashMap<InternedId, ModuleId>,
    /// All modules that were found by `module_finder`
    pub mods: Vec<Module>,
    /// Type table which contains every module's seen types
    pub types: Vec<TypeInfo>,
    /// All values that were cached
    pub values: Vec<ValueInfo>,
    /// All expressions that were found
    pub exprs: Vec<ResolvedExpr>,
    // pub exprs: Vec<ValueInfo>,
    pub symbols: HashMap<SymbolId, Symbol>,
    ///
    pub scopes: Vec<ScopeInfo>,
    /// Module id for the stdlib which is always pre-loaded
    pub core_mod_id: ModuleId,
}

// Called idx but is u32...
pub const TYPE_UNKNOWN_IDX: u32 = CORE_ANY + 1;

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
pub const CORE_ANY: u32 = 22;
pub const CORE_LIST: u32 = 23;
pub const CORE_SET: u32 = 24;
pub const CORE_MAP: u32 = 25;
pub const CORE_TUPLE: u32 = 26;

// Helper struct
// struct ScriptStdLib {}

// ----
// NOTE: May turn this into an innate option type inside of HIR
// Ok now this really needs to be an option
pub const VALUE_UNKNOWN: usize = 0;

impl ScriptCompiler {
    //FIX: Arbitrary ordering of pushes tied to the actual order of the enums. Should not be tied
    //to anything, similar to the interner's constants.
    /// Loads std and builds script specific compiler with parameters given
    pub fn new(
        bind: Option<Bind>,
        mod_map: HashMap<InternedId, ModuleId>,
        mods: Vec<Module>,
    ) -> ScriptCompiler {
        //TEST:
        let core_mod_id = mods.len();
        let mut script_compiler = ScriptCompiler {
            bind,
            mod_map,
            mods,
            types: Vec::new(),
            values: Vec::new(),
            exprs: Vec::new(),
            symbols: HashMap::new(),
            scopes: Vec::new(),
            //TEST:
            core_mod_id: ModuleId::new(core_mod_id),
        };

        Self::load_core(&mut script_compiler);

        script_compiler
    }
    pub(super) fn get_typedef(&self, sym_id: SymbolId) -> &TypeDef {
        match &self.symbols[&sym_id] {
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
        match &self.symbols.get_mut(&sym_id).expect("misusage") {
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
        match &self.symbols[&sym_id] {
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
        match self.symbols.get_mut(&sym_id).expect("misusage") {
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
        match &self.symbols[&sym_id] {
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
        match self.symbols.get_mut(&sym_id).expect("misusage") {
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
        match &self.symbols[&sym_id] {
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
        match self.symbols.get_mut(&sym_id).expect("misusage") {
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
        match &self.symbols[&sym_id] {
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
        match self.symbols.get_mut(&sym_id).expect("Misusage") {
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
        match &self.symbols[&sym_id] {
            sym_info => match &sym_info.kind {
                SymbolKind::Val(val_id) => &self.values[val_id.id as usize],
                _ => unreachable!(),
            },
        }
    }

    /// Assumes the symbol given is a variable, meaning a symbol with a value inside of it
    pub(super) fn get_var_mut(&mut self, sym_id: SymbolId) -> &mut ValueInfo {
        match &self.symbols[&sym_id] {
            sym_info => match &sym_info.kind {
                SymbolKind::Val(val_id) => &mut self.values[val_id.id as usize],
                _ => unreachable!(),
            },
        }
    }

    /// Returns `ModuleId` which is the module of origin
    pub fn get_owner(&self, sym_id: SymbolId) -> ModuleId {
        self.symbols[&sym_id].owner
    }

    /// Get's the `ScopeId` with no assumption of it existing.
    ///
    /// This method exists along with extract_scope_id due to cross module namespace checking not
    /// innately confirming whether or not it contains a particular `ScopeType`
    pub fn get_scope_id(&self, scope_type: ScopeType, owner: &Module) -> Option<ScopeId> {
        self.find_scope(scope_type, owner).map(|s| s.scope.scope_id)
    }

    /// Get's the `ScopeId` assuming that the scope already exists. Panics otherwise.
    ///
    /// This exists because if the current module has something like a typedef in the semantic stage,
    /// that means the parser itself already checked if it was legal grammar-wise.
    pub fn extract_scope_id(&self, scope_type: ScopeType, owner_id: ModuleId) -> ScopeId {
        let owner_mod = &self.mods[owner_id.id];
        self.find_scope(scope_type, owner_mod)
            .expect("Either semantic broke, parser broke, or modules broke")
            .scope
            .scope_id
    }

    /// Get's scope using a `ScopeId`
    pub fn get_scope(&self, scope_id: ScopeId) -> &ScopeInfo {
        &self.scopes[scope_id.id]
    }

    /// Returns mutably borrowed scope using a `ScopeId`
    pub fn get_scope_mut(&mut self, scope_id: ScopeId) -> &mut ScopeInfo {
        &mut self.scopes[scope_id.id]
    }

    /// Pushes new scope with given scope type and returns the `ScopeId`. If the scope already
    /// exists then it returns the existent `ScopeId`.
    pub fn push_scope(&mut self, scope_type: ScopeType, owner_id: ModuleId) -> ScopeId {
        let owner_mod = &self.mods[owner_id.id];
        if let Some(scope_info) = self.find_scope(scope_type, owner_mod) {
            return scope_info.scope.scope_id;
        }

        let scope_id = ScopeId::new(self.scopes.len());
        let scope = Scope::new(scope_id, scope_type);
        let scope_info = ScopeInfo::new(scope, owner_id);
        self.scopes.push(scope_info);

        let owner_mod = &mut self.mods[owner_id.id];
        owner_mod.scopes.push(scope_id);
        // BRAIN OFF

        scope_id
    }

    /// Checks if the name id corresponds to a `SymbolId` within the given `ScopeType`.
    /// Returns a tuple of the `AstId` and `ScopeType` the `NameId` was found in. Returns None if
    /// no accessible scopes contain the given `NameId`.
    pub fn get_sym_id(
        &self,
        name_id: InternedId,
        scope_type: ScopeType,
        mod_owner: &Module,
    ) -> Option<SymbolId> {
        // I don't think this can fail. Should maybe expect for clarity.
        for scope_id in &mod_owner.scopes {
            let scope = &self.scopes[scope_id.id].scope;
            // Loops over all allowed scopes and checks their individual namespaces
            for allowed_scope_type in scope.accessible_scopes.iter().copied() {
                // In this scenario the scope may or may not exist since this could be used from
                // another module
                if let Some(scope_info) = self.find_scope(allowed_scope_type, mod_owner) {
                    for (current_ast_id, current_name_id) in &scope_info.scope.table.name_ids {
                        if *current_name_id == name_id {
                            let scope_id =
                                self.extract_scope_id(allowed_scope_type, mod_owner.mod_id);
                            let scope_info = self.get_scope(scope_id);

                            let sym_id = scope_info.scope.table.ast_to_sym[&current_ast_id];
                            return Some(sym_id);
                        }
                    }
                }
            }
        }
        //TEST: If all scopes fail

        None
    }

    /// Returns Some scope if it exists, None otherwise
    //NOTE: May opt for indices similarly to the ast's way of making sections
    pub fn find_scope(&self, scope_type: ScopeType, mod_owner: &Module) -> Option<&ScopeInfo> {
        for scope_id in &mod_owner.scopes {
            let scope_info = &self.scopes[scope_id.id];
            if scope_info.scope.scope_type == scope_type {
                return Some(scope_info);
            }
        }

        None
    }

    fn load_core(compiler: &mut ScriptCompiler) {
        let mut table = Table::new();

        //TODO: If namespace core exists as a module then should error earlier
        let core_name = InternedId::new(intern::INTERNED_CORE);
        let core_mod_id = ModuleId::new(compiler.mods.len());
        let core_scope_id = ScopeId::new(compiler.scopes.len());
        let mut core_mod = Module::new(core_name, core_mod_id, Vec::new(), None);

        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::I8),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        // Added this interned id part
        let interned_id = InternedId::new(intern::INTERNED_I8);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_I8)),
        );

        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

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
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_U8)),
        );

        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

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
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_I16)),
        );
        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

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
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_U16)),
        );
        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

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
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_F16)),
        );
        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

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
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_I32)),
        );
        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

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
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_U32)),
        );
        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

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
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_F32)),
        );
        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

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
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_I64)),
        );
        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

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
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_U64)),
        );
        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

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
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_F64)),
        );
        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

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
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_I128)),
        );
        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

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
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_U128)),
        );
        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

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
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_F128)),
        );
        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

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
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_SIZED)),
        );
        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

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
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_UNSIZED)),
        );
        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

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
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_STR)),
        );
        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

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
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_CHAR)),
        );
        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

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
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_NIL)),
        );
        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

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
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_BOOL)),
        );
        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

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
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_BIGINT)),
        );
        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

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
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_BIGFLOAT)),
        );
        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);

        compiler.types.push(TypeInfo::new(
            Type::BuiltinType(BuiltinType::Any),
            core_mod_id,
        ));

        let sym_id = SymbolId::new(compiler.symbols.len() as u32);
        let interned_id = InternedId::new(intern::INTERNED_ANY);
        let symbol = Symbol::new(
            interned_id,
            sym_id,
            None,
            core_mod_id,
            false,
            ScopeType::Core,
            SymbolKind::Type(TypeId::new(CORE_ANY)),
        );

        compiler.symbols.insert(sym_id, symbol);
        table.interned_to_sym.insert(interned_id, sym_id);
        compiler
            .types
            .push(TypeInfo::new(Type::Unknown, core_mod_id));

        let scope_id = ScopeId::new(compiler.scopes.len());
        let scope = Scope::with_table(scope_id, ScopeType::Core, table);
        let scope_info = ScopeInfo::new(scope, core_mod_id);

        compiler.scopes.push(scope_info);
        core_mod.scopes.push(scope_id);

        compiler.mod_map.insert(core_name, core_mod_id);
        compiler.mods.push(core_mod);

        for module in &mut compiler.mods {
            module.scopes.push(core_scope_id);
        }

        //TODO: Maybe global scope table with the root being std since this std_lib_id stuff is a
        //bit fewfijewf
    }
}
