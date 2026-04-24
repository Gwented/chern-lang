use std::collections::HashMap;

use chern_core::{
    builtins::{self, BuiltinType, BuiltinTypeKind},
    id_types::{InternedId, ModuleId, SymbolId, TypeId},
    intern, keywords,
    values::{Value, ValueInfo},
};

use crate::{
    modules::{Bind, Module},
    semantic::representation::{
        self, EnumDef, FuncRepre, ResolvedExpr, StructDef, Symbol, SymbolKind, Type, TypeDef,
        TypeInfo,
    },
};

pub struct ScriptCompiler {
    /// Optional bind statement that is obtained from the main module
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
    pub(crate) symbols: HashMap<SymbolId, Symbol>,
}

// ----
pub const TYPE_UNKNOWN_IDX: u32 = BuiltinTypeKind::BigFloat as u32 + 1;

// ----
pub const VALUE_FALSE_POS: usize = 0;
pub const VALUE_TRUE_POS: usize = 1;
// NOTE: May turn this into an innate option type inside of HIR
pub const VALUE_UNKNOWN_POS: usize = 2;

impl ScriptCompiler {
    //FIX: Arbitrary ordering of pushes tied to the actual order of the enums. Should not be tied
    //to anything, similar to the interner's constants.
    pub fn new(
        bind: Option<Bind>,
        mod_map: HashMap<InternedId, ModuleId>,
        mods: Vec<Module>,
    ) -> ScriptCompiler {
        let mut types: Vec<TypeInfo> = Vec::new();

        // Pre-loading keywords
        // If this fails something was messed up within keywords itself
        //
        // This must be subtracted or else it will include builtin types that are data structures
        // which are of course not pre-loadable.
        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_I8).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_U8).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_I16).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_U16).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_F16).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_I32).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_U32).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_F32).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_I64).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_U64).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_F64).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_I128).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_U128).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_F128).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_SIZED).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty = BuiltinType::try_from_interned_id(intern::INTERNED_UNSIZED)
            .expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_STR).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_CHAR).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_NIL).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_BOOL).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_BIGINT).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty = BuiltinType::try_from_interned_id(intern::INTERNED_BIGFLOAT)
            .expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        types.push(TypeInfo::new(Type::Unknown, None));

        let values: Vec<ValueInfo> = Vec::new();
        // let val_info = ValueInfo::new
        // values.push(Value::Bool(false));
        // values.push(Value::Bool(true));
        // values.push(Value::Unknown);

        ScriptCompiler {
            bind,
            mod_map,
            mods,
            types,
            values,
            exprs: Vec::new(),
            symbols: HashMap::new(),
        }
    }

    // Is there a reason to return err?
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

    pub(super) fn get_func(&self, sym_id: SymbolId) -> &FuncRepre {
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

    pub(super) fn get_func_mut(&mut self, sym_id: SymbolId) -> &mut FuncRepre {
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

    // pub(super) fn get_var(&self, sym_id: SymbolId) -> &VarDef {
    //     match &self.symbols[&sym_id] {
    //         sym_info => match &sym_info.kind {
    //             SymbolKind::Type(type_id) => match &self.types[type_id.id as usize].ty {
    //                 Type::Var(var_def) => enum_def,
    //                 _ => unreachable!(),
    //             },
    //             _ => unreachable!(),
    //         },
    //     }
    // }
    //
    // pub(super) fn get_var_mut(&mut self, sym_id: SymbolId) -> &mut VarDef {
    //     match self.symbols.get_mut(&sym_id) {
    //         Some(sym_info) => match &mut sym_info.symbol {
    //             Symbol::Var(var_repre) => var_repre,
    //             _ => unreachable!(),
    //         },
    //         None => unreachable!(),
    //     }
    // }

    pub(super) fn get_owner(&self, sym_id: SymbolId) -> ModuleId {
        self.symbols[&sym_id].owner
    }
}
