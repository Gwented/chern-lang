// TypeId is the index of the type itself, OR the type it's pointing to
use core::error;
use std::{collections::HashMap, fmt::Display};

use common::{
    builtins::{BuiltinType, BuiltinTypeKind},
    fmter::{Formattable, Formatted},
    keywords,
    symbols::{
        AstId, BuiltinTypeId, EnumId, FuncId, InnerArgs, NameId, Span, SpannedInnerArgs, StructId,
        SymbolId, TypeDefId, TypeId,
    },
};

use crate::{semantic::constraints::ArgConstraint, types::symbols::Cond};

// What is a drop? I am new to thinking i have never thought before what is RAII
// is that a gui framework
// Maybe named, global table, program table

#[derive(Debug)]
pub(super) enum Type {
    BuiltinType(BuiltinType),
    Struct(SymbolId),
    Enum(SymbolId),
    Func(SymbolId),
    Alias(SymbolId),
    Const(SymbolId),
    Tuple(Tuple),
    Unknown,
}

#[derive(Debug)]
pub(super) enum Symbol {
    TypeDef(TypeDefRepre),
    Struct(StructRepre),
    Func(FuncRepre),
    Enum(EnumRepre),
    Alias(AliasRepre),
    Const(ConstRepre),
}

#[derive(Debug)]
pub struct Table {
    pub(super) name_ids: HashMap<AstId, NameId>,
    // Can still change some to vec maybe
    pub(super) sym_ids: HashMap<AstId, SymbolId>,
    pub(super) symbols: HashMap<SymbolId, Symbol>,
    pub(super) types: Vec<Type>,
}

impl Table {
    pub fn new() -> Table {
        let mut table = Table {
            name_ids: HashMap::new(),
            sym_ids: HashMap::new(),
            symbols: HashMap::new(),
            types: Vec::new(),
        };

        // TEST: Taking away the data structures with - 3
        for i in 0..keywords::TYPE_END - 3 {
            let ty = BuiltinType::try_from_id(i as u32).expect("Builtin type not updated");
            table.types.push(Type::BuiltinType(ty));
        }

        table
    }

    // Is there a reason to return err?
    pub(super) fn get_typedef(&self, sym_id: SymbolId) -> &TypeDefRepre {
        match &self.symbols[&sym_id] {
            symbol => match symbol {
                Symbol::TypeDef(type_def_repre) => type_def_repre,
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_typedef_mut(&mut self, sym_id: SymbolId) -> &mut TypeDefRepre {
        match self.symbols.get_mut(&sym_id) {
            Some(symbol) => match symbol {
                Symbol::TypeDef(type_def_repre) => type_def_repre,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }

    pub(super) fn get_struct(&self, sym_id: SymbolId) -> &StructRepre {
        match self.symbols.get(&sym_id) {
            Some(symbol) => match symbol {
                Symbol::Struct(struct_repre) => struct_repre,
                _ => unreachable!(),
            },
            None => unreachable!(),
        }
    }

    pub(super) fn get_struct_mut(&mut self, sym_id: SymbolId) -> &mut StructRepre {
        match self.symbols.get_mut(&sym_id) {
            Some(symbol) => match symbol {
                Symbol::Struct(struct_repre) => struct_repre,
                _ => unreachable!(),
            },
            None => unreachable!(),
        }
    }

    pub(super) fn get_func(&self, sym_id: SymbolId) -> &FuncRepre {
        match &self.symbols[&sym_id] {
            symbol => match symbol {
                Symbol::Func(func_repre) => func_repre,
                // Symbol::TypeDef(type_def_repre) => todo!(),
                // Symbol::Struct(struct_repre) => Some(struct_repre),
                // Symbol::Enum(enum_repre) => todo!(),
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_func_mut(&mut self, sym_id: SymbolId) -> &mut FuncRepre {
        match self.symbols.get_mut(&sym_id) {
            Some(symbol) => match symbol {
                Symbol::Func(func_repre) => func_repre,
                // Symbol::TypeDef(type_def_repre) => todo!(),
                // Symbol::Struct(struct_repre) => Some(struct_repre),
                // Symbol::Enum(enum_repre) => todo!(),
                _ => unreachable!(),
            },
            None => unreachable!(),
        }
    }

    pub(super) fn get_enum(&self, sym_id: SymbolId) -> &EnumRepre {
        match &self.symbols[&sym_id] {
            symbol => match symbol {
                Symbol::Enum(enum_repre) => enum_repre,
                // Symbol::Func(func_repre) => Some(func_repre),
                // Symbol::TypeDef(type_def_repre) => todo!(),
                // Symbol::Struct(struct_repre) => Some(struct_repre),
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_enum_mut(&mut self, sym_id: SymbolId) -> &mut EnumRepre {
        match self.symbols.get_mut(&sym_id) {
            Some(symbol) => match symbol {
                Symbol::Enum(enum_repre) => enum_repre,
                // Symbol::Func(func_repre) => Some(func_repre),
                // Symbol::TypeDef(type_def_repre) => todo!(),
                // Symbol::Struct(struct_repre) => Some(struct_repre),
                _ => unreachable!(),
            },
            None => unreachable!(),
        }
    }
}

#[derive(Debug)]
pub(super) struct ConstRepre {
    pub(super) name_id: NameId,
    pub(super) sym_id: SymbolId,
    pub(super) ast_id: AstId,
    // It's position in the Type array
    pub(super) type_id: TypeId,
}

impl ConstRepre {
    pub fn new(name_id: NameId, sym_id: SymbolId, ast_id: AstId, type_id: TypeId) -> ConstRepre {
        ConstRepre {
            name_id,
            sym_id,
            ast_id,
            type_id,
        }
    }
}

#[derive(Debug)]
pub(super) struct StructRepre {
    pub(super) name_id: NameId,
    pub(super) sym_id: SymbolId,
    pub(super) ast_id: AstId,
    // It's position in the Type array
    pub(super) type_id: TypeId,
    pub(super) fields: Vec<FieldRepre>,
    pub(super) args: Vec<InnerArgs>,
    pub(super) conds: Vec<Cond>,
}

impl StructRepre {
    pub fn new(
        name_id: NameId,
        sym_id: SymbolId,
        ast_id: AstId,
        type_id: TypeId,
        fields: Vec<FieldRepre>,
    ) -> StructRepre {
        StructRepre {
            name_id,
            sym_id,
            ast_id,
            type_id,
            fields,
            args: Vec::new(),
            conds: Vec::new(),
        }
    }

    // Likely too complex to be handled inside like this and should maybe be given a baked version
    // so that it can focus on checking arg types or the keyword of the cond.
    // pub fn supports_cond(&self, cond: Cond) -> bool {
    //     match cond {
    //         Cond::IsEmpty => todo!(),
    //         Cond::IsWhitespace => todo!(),
    //         Cond::Func(func_id) => todo!(),
    //         Cond::Not(cond) => todo!(),
    //     }
    // }
}

#[derive(Debug)]
pub(super) struct EnumRepre {
    pub(super) name_id: NameId,
    // Unsure about this positioning, I am hallucinating.
    pub(super) sym_id: SymbolId,
    pub(super) ast_id: AstId,
    pub(super) type_id: TypeId,
    pub(super) variants: Vec<VariantRepre>,
    pub(super) args: Vec<InnerArgs>,
    pub(super) conds: Vec<Cond>,
}

impl EnumRepre {
    pub fn new(
        name_id: NameId,
        sym_id: SymbolId,
        ast_id: AstId,
        type_id: TypeId,
        variants: Vec<VariantRepre>,
    ) -> EnumRepre {
        EnumRepre {
            name_id,
            sym_id,
            ast_id,
            type_id,
            variants,
            args: Vec::new(),
            conds: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct VariantRepre {
    pub(super) name_id: NameId,
    // Because enum types are nullable
    pub(super) type_id: Option<TypeId>,
    // Possible tuple
    // Points to variant within original Ast enum
    pub(super) ast_id: AstId,
    pub(super) args: Vec<InnerArgs>,
    pub(super) conds: Vec<Cond>,
}

impl VariantRepre {
    pub fn new(name_id: NameId, type_id: Option<TypeId>, ast_id: AstId) -> VariantRepre {
        VariantRepre {
            name_id,
            type_id,
            ast_id,
            args: Vec::new(),
            conds: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub(super) struct TypeDefRepre {
    pub(super) name_id: NameId,
    pub(super) sym_id: SymbolId,
    pub(super) ast_id: AstId,
    pub(super) type_id: TypeId,
    pub(super) conds: Vec<Cond>,
    pub(super) args: Vec<InnerArgs>,
}

impl TypeDefRepre {
    pub fn new(name_id: NameId, type_id: TypeId, sym_id: SymbolId, ast_id: AstId) -> TypeDefRepre {
        TypeDefRepre {
            name_id,
            sym_id,
            ast_id,
            type_id,
            conds: Vec::new(),
            args: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct FuncRepre {
    pub(super) name_id: NameId,
    // Type reference to it's own position in the `Type` array
    pub(super) type_id: TypeId,
    pub(super) call_span: Span,
    pub(super) kind: FuncKind,
    pub(super) constraints: Vec<ArgConstraint>,
    pub(super) args: Vec<FuncArgsRepre>,
}

impl FuncRepre {
    pub(super) fn new(
        name_id: NameId,
        type_id: TypeId,
        call_span: Span,
        kind: FuncKind,
        constraints: Vec<ArgConstraint>,
        args: Vec<FuncArgsRepre>,
    ) -> FuncRepre {
        FuncRepre {
            name_id,
            type_id,
            kind,
            call_span,
            constraints,
            args,
        }
    }
}

// I'm scared of this
#[derive(Debug)]
pub(crate) enum FuncArgsRepre {
    Integer(i64),
    Float(f64),
    Char(char),
    //TEST:
    Var(TypeId, BuiltinTypeKind),
    Default(TypeId, BuiltinTypeKind, Box<FuncArgsRepre>),
    Str(NameId),
}

impl FuncArgsRepre {
    pub(crate) fn is_numeric(&self) -> bool {
        match self {
            FuncArgsRepre::Integer(_) | FuncArgsRepre::Float(_) => true,
            FuncArgsRepre::Char(_) | FuncArgsRepre::Str(_) => false,
            FuncArgsRepre::Var(_, kind) | FuncArgsRepre::Default(_, kind, _) => kind.is_numeric(),
        }
    }

    // TEST: Currently testing out a way of formatting that is more encapsulated so that the code
    // outside doesn't have to be repeated as intensely.
    pub(crate) fn is_integer(&self) -> bool {
        match self.kind() {
            FuncArgsKind::Integer => true,
            _ => false,
        }
    }

    pub(crate) fn is_float(&self) -> bool {
        match self.kind() {
            FuncArgsKind::Float => true,
            _ => false,
        }
    }

    pub(crate) fn is_char(&self) -> bool {
        match self.kind() {
            FuncArgsKind::Char => true,
            _ => false,
        }
    }

    pub(crate) fn is_str(&self) -> bool {
        match self.kind() {
            FuncArgsKind::Str => true,
            _ => false,
        }
    }

    // to_builtin_type_kind is getting a little long for something so contextually obvious
    pub(crate) fn to_builtin_kind(&self) -> BuiltinTypeKind {
        match self {
            FuncArgsRepre::Integer(_) => BuiltinTypeKind::I64,
            FuncArgsRepre::Float(_) => BuiltinTypeKind::F64,
            FuncArgsRepre::Char(_) => BuiltinTypeKind::Char,
            FuncArgsRepre::Var(_, kind) | FuncArgsRepre::Default(_, kind, _) => *kind,
            FuncArgsRepre::Str(_) => BuiltinTypeKind::Str,
        }
    }

    pub(crate) fn kind(&self) -> FuncArgsKind {
        match self {
            FuncArgsRepre::Integer(_) => FuncArgsKind::Integer,
            FuncArgsRepre::Float(_) => FuncArgsKind::Float,
            FuncArgsRepre::Char(_) => FuncArgsKind::Char,
            FuncArgsRepre::Var(_, _) => FuncArgsKind::Var,
            FuncArgsRepre::Str(_) => FuncArgsKind::Str,
            FuncArgsRepre::Default(_, _, _) => FuncArgsKind::Default,
        }
    }
}

impl Formattable for FuncArgsRepre {
    fn to_fmt(&self) -> Formatted {
        match self {
            FuncArgsRepre::Integer(_) => Formatted::Integer,
            FuncArgsRepre::Float(_) => Formatted::Float,
            FuncArgsRepre::Char(_) => Formatted::Char,
            FuncArgsRepre::Var(_, kind) | FuncArgsRepre::Default(_, kind, _) => kind.to_fmt(),
            FuncArgsRepre::Str(_) => Formatted::Str,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FuncArgsKind {
    Integer,
    Float,
    Char,
    Var,
    Default,
    Str,
}

#[derive(Debug)]
pub(super) struct FieldRepre {
    pub(super) name_id: NameId,
    pub(super) type_id: TypeId,
    // Ast contained field id, maybe this should just be AstId
    pub(super) ast_id: AstId,
}

impl FieldRepre {
    pub fn new(name_id: NameId, type_id: TypeId, ast_id: AstId) -> FieldRepre {
        FieldRepre {
            name_id,
            type_id,
            ast_id,
        }
    }
}

#[derive(Debug)]
pub(super) struct AliasRepre {
    pub(crate) name_id: NameId,
    pub(super) sym_id: SymbolId,
    pub(super) ast_id: AstId,
    // Refers to self's type id position
    // Maybe call this type_addr?
    pub(super) type_id: TypeId,
    pub(crate) params: Vec<TypeId>,
    pub(super) conds: Vec<Cond>,
    pub(super) args: Vec<InnerArgs>,
}

impl AliasRepre {
    pub fn new(
        name_id: NameId,
        sym_id: SymbolId,
        ast_id: AstId,
        type_id: TypeId,
        params: Vec<TypeId>,
        conds: Vec<Cond>,
        args: Vec<InnerArgs>,
    ) -> AliasRepre {
        AliasRepre {
            name_id,
            sym_id,
            ast_id,
            type_id,
            params,
            conds,
            args,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FuncKind {
    Contains,
    Range,
    StartsW,
    EndsW, // UserDefined
    Equals,
    UserDefined,
}

impl Formattable for FuncKind {
    fn to_fmt(&self) -> common::fmter::Formatted {
        match self {
            FuncKind::Contains => Formatted::FuncContains,
            FuncKind::Range => Formatted::FuncRange,
            FuncKind::StartsW => Formatted::FuncStartsW,
            FuncKind::EndsW => Formatted::FuncEndsW,
            FuncKind::UserDefined => Formatted::UserFunc,
            FuncKind::Equals => Formatted::FuncEquals,
        }
    }
}

impl Display for FuncKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FuncKind::Contains => write!(f, "Contains"),
            FuncKind::Range => write!(f, "Range"),
            FuncKind::StartsW => write!(f, "StartsW"),
            FuncKind::EndsW => write!(f, "EndsW"),
            FuncKind::UserDefined => write!(f, "<Hi>"),
            FuncKind::Equals => write!(f, "Equals"),
        }
    }
}

#[derive(Debug)]
pub(super) struct Tuple {
    pub(super) elements: Vec<TypeId>,
    pub(super) type_id: TypeId,
}

impl Tuple {
    pub fn new(elements: Vec<TypeId>, type_id: TypeId) -> Tuple {
        Tuple { elements, type_id }
    }
}
