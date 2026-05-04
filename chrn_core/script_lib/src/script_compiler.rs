use std::collections::HashMap;

use chrn_utils::{
    builtins::{BuiltinType, BuiltinTypeKind},
    id_types::{InternedId, ModuleId, SymbolId},
    intern,
    values::ValueInfo,
};

use crate::{
    modules::{Bind, Module},
    semantic::representation::{
        AliasDef, EnumDef, FuncDef, ResolvedExpr, StructDef, Symbol, SymbolKind, Type, TypeDef,
        TypeInfo,
    },
};

//TODO: Intrinsic scope that holds value, type, etc. tables that is just a scope that is innate by
//default, which can be explicitly referenced. Like, intrinsic.str is just the default str
//identification, so no escape needed anymore for that
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
    /// Module id for the stdlib which is always pre-loaded
    pub std_mod_id: ModuleId,
}

// Called idx but is u32...
pub const TYPE_UNKNOWN_IDX: u32 = BuiltinTypeKind::BigFloat as u32 + 1;
// pub const TYPE_UNKNOWN_IDX: u32 = STD_BIGFLOAT + 1;

pub const STD_I8: u32 = 0;
pub const STD_U8: u32 = 1;
pub const STD_I16: u32 = 2;
pub const STD_U16: u32 = 3;
pub const STD_F16: u32 = 4;
pub const STD_I32: u32 = 5;
pub const STD_U32: u32 = 6;
pub const STD_F32: u32 = 7;
pub const STD_I64: u32 = 8;
pub const STD_U64: u32 = 9;
pub const STD_F64: u32 = 10;
pub const STD_I128: u32 = 11;
pub const STD_U128: u32 = 12;
pub const STD_F128: u32 = 13;
pub const STD_SIZED: u32 = 14;
pub const STD_UNSIZED: u32 = 15;
pub const STD_STR: u32 = 16;
pub const STD_CHAR: u32 = 17;
pub const STD_NIL: u32 = 18;
pub const STD_BOOL: u32 = 19;
pub const STD_BIGINT: u32 = 20;
pub const STD_BIGFLOAT: u32 = 21;
pub const STD_LIST: u32 = 22;
pub const STD_SET: u32 = 23;
pub const STD_MAP: u32 = 24;
pub const STD_TUPLE: u32 = 25;
pub const STD_ANY: u32 = 26;

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
        mut mod_map: HashMap<InternedId, ModuleId>,
        mut mods: Vec<Module>,
    ) -> ScriptCompiler {
        let (std_mod_id, types) = Self::load_std(&mut mod_map, &mut mods);

        ScriptCompiler {
            bind,
            mod_map,
            mods,
            types,
            values: Vec::new(),
            exprs: Vec::new(),
            symbols: HashMap::new(),
            std_mod_id,
        }
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
    pub(super) fn get_owner(&self, sym_id: SymbolId) -> ModuleId {
        self.symbols[&sym_id].owner
    }

    fn load_std(
        mod_map: &mut HashMap<InternedId, ModuleId>,
        mods: &mut Vec<Module>,
    ) -> (ModuleId, Vec<TypeInfo>) {
        let mut types: Vec<TypeInfo> = Vec::new();

        //TODO: If namespace std exists as a module then should error earlier
        let std_name = InternedId::new(intern::INTERNED_STD);
        let std_mod_id = ModuleId::new(mods.len());
        let std_mod = Module::new(std_name, std_mod_id, Vec::new(), None);

        mod_map.insert(std_name, std_mod_id);
        mods.push(std_mod);

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_I8).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_U8).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_I16).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_U16).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_F16).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_I32).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_U32).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_F32).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_I64).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_U64).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_F64).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_I128).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_U128).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_F128).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_SIZED).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        let ty = BuiltinType::try_from_interned_id(intern::INTERNED_UNSIZED)
            .expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_STR).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_CHAR).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_NIL).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_BOOL).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_BIGINT).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        let ty = BuiltinType::try_from_interned_id(intern::INTERNED_BIGFLOAT)
            .expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), std_mod_id));

        types.push(TypeInfo::new(Type::Unknown, std_mod_id));

        (std_mod_id, types)
    }
}
