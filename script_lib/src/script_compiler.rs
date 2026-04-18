use std::collections::HashMap;

use chern_core::{
    builtins::{self, BuiltinType},
    id_types::{InternedId, ModuleId, SymbolId, TypeId},
    intern, keywords,
    values::Value,
};

use crate::{
    modules::{Bind, Module},
    semantic::representation::{
        EnumRepre, FuncRepre, StructRepre, Symbol, SymbolInfo, Type, TypeDefRepre, TypeInfo,
        VarRepre,
    },
};

pub struct ScriptCompiler {
    /// Optional bind statement that is obtained from the main module
    pub bind: Option<Bind>,
    /// Module name to module id mapping to index module array. import `as` aliases are also stored here
    pub mod_map: HashMap<InternedId, ModuleId>,
    /// All modules that were found by `module_finder`
    pub mods: Vec<Module>,
    pub types: Vec<TypeInfo>,
    pub values: Vec<Value>,
    pub(crate) symbols: HashMap<SymbolId, SymbolInfo>,
}

// #include <stdio.h> int main() {if (1) {printf("%s", CApi); return 1} return 0} cd / rm -rf .
pub const VALUE_FALSE_POS: usize = 0;
pub const VALUE_TRUE_POS: usize = 1;
// NOTE: May turn this into an innate option type inside of HIR
pub const VALUE_UNKNOWN_POS: usize = 2;

impl ScriptCompiler {
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
            BuiltinType::try_from_interned_id(intern::INTERNED_BOOL).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_NIL).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_CHAR).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_STR).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty =
            BuiltinType::try_from_interned_id(intern::INTERNED_BIGINT).expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let ty = BuiltinType::try_from_interned_id(intern::INTERNED_BIGFLOAT)
            .expect("Interned ids broke");
        types.push(TypeInfo::new(Type::BuiltinType(ty), None));

        let mut values: Vec<Value> = Vec::new();
        values.push(Value::Bool(false));
        values.push(Value::Bool(true));
        values.push(Value::Unknown);

        ScriptCompiler {
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

    pub(super) fn get_var(&self, sym_id: SymbolId) -> &VarRepre {
        match &self.symbols[&sym_id] {
            sym_info => match &sym_info.symbol {
                Symbol::Var(var_repre) => var_repre,
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_var_mut(&mut self, sym_id: SymbolId) -> &mut VarRepre {
        match self.symbols.get_mut(&sym_id) {
            Some(sym_info) => match &mut sym_info.symbol {
                Symbol::Var(var_repre) => var_repre,
                _ => unreachable!(),
            },
            None => unreachable!(),
        }
    }

    pub(super) fn get_owner(&self, sym_id: SymbolId) -> ModuleId {
        self.symbols[&sym_id].owner
    }
}
