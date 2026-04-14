use std::collections::HashMap;

use chern_core::{
    builtins::BuiltinType,
    id_types::{ModuleId, NameId, SymbolId},
    keywords,
    values::Value,
};

use crate::{
    modules::Module,
    parser::ast::Bind,
    semantic::representation::{
        ConstRepre, EnumRepre, FuncRepre, StructRepre, Symbol, SymbolInfo, Type, TypeDefRepre,
        TypeInfo,
    },
};

pub struct ScriptCompiler {
    /// Optional bind statement that is obtained from the main module
    pub bind: Option<Bind>,
    /// Module name to module id mapping to index module array. import `as` aliases are also stored here
    pub mod_map: HashMap<NameId, ModuleId>,
    /// All modules that were found by `module_finder`
    pub mods: Vec<Module>,
    pub types: Vec<TypeInfo>,
    pub values: Vec<Value>,
    pub(crate) symbols: HashMap<SymbolId, SymbolInfo>,
}

// #include <stdio.h> int main() {if (1) {printf("%s", CApi); return 1} return 0} cd / rm -rf .
pub const VALUE_FALSE_POS: usize = 0;
pub const VALUE_TRUE_POS: usize = 1;
pub const VALUE_UNKNOWN_POS: usize = 2;

impl ScriptCompiler {
    pub fn new(
        bind: Option<Bind>,
        mod_map: HashMap<NameId, ModuleId>,
        mods: Vec<Module>,
    ) -> ScriptCompiler {
        let mut types: Vec<TypeInfo> = Vec::new();

        // Pre-loading keywords
        // If this fails something was messed up within keywords itself
        for i in 0..keywords::TYPE_END - 4 {
            let ty = BuiltinType::try_from_id(i as u32).expect("Builtin type not updated");
            types.push(TypeInfo::new(Type::BuiltinType(ty), None));
        }

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
