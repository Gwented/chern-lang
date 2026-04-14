// TypeId is the index of the type itself, OR the type it's pointing to
use std::{collections::HashMap, fmt::Display};

use chern_core::{
    builtins::{BuiltinType, BuiltinTypeKind},
    id_types::{AstId, ModuleId, NameId, SymbolId, TypeId, ValueId},
    inner_args::InnerArgs,
};
use common::{
    fmter::{Formattable, Formatted},
    span::Span,
};

use crate::{conditions::Cond, semantic::constraints::ArgConstraint};

// What is a drop? I am new to thinking i have never thought before what is RAII
// is that a gui framework
// Maybe named, global table, program table

#[derive(Debug)]
pub struct TypeInfo {
    pub ty: Type,
    pub owner: Option<ModuleId>,
}

impl TypeInfo {
    pub fn new(ty: Type, owner: Option<ModuleId>) -> TypeInfo {
        TypeInfo { ty, owner }
    }
}

#[derive(Debug)]
pub struct SymbolInfo {
    pub symbol: Symbol,
    pub owner: ModuleId,
    pub is_priv: bool,
}

impl SymbolInfo {
    pub fn new(symbol: Symbol, is_priv: bool, owner: ModuleId) -> SymbolInfo {
        SymbolInfo {
            symbol,
            is_priv,
            owner,
        }
    }
}

#[derive(Debug)]
pub enum Type {
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
pub(crate) enum Symbol {
    TypeDef(TypeDefRepre),
    Struct(StructRepre),
    Func(FuncRepre),
    Enum(EnumRepre),
    Alias(AliasRepre),
    Const(ConstRepre),
}

#[derive(Debug)]
pub struct Table {
    // Type specific tables
    pub(crate) name_ids: HashMap<AstId, NameId>,
    // Can still change some to vec maybe
    pub(crate) sym_ids: HashMap<AstId, SymbolId>,
}

impl Table {
    pub fn new() -> Table {
        Table {
            name_ids: HashMap::new(),
            sym_ids: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct ConstRepre {
    pub(crate) name_id: NameId,
    pub(crate) sym_id: SymbolId,
    pub(crate) ast_id: AstId,
    // It's position in the Type array
    pub(crate) value_id: ValueId,
}

impl ConstRepre {
    pub fn new(name_id: NameId, sym_id: SymbolId, ast_id: AstId, value_id: ValueId) -> ConstRepre {
        ConstRepre {
            name_id,
            sym_id,
            ast_id,
            value_id,
        }
    }
}

#[derive(Debug)]
pub(crate) struct StructRepre {
    pub(crate) name_id: NameId,
    pub(crate) sym_id: SymbolId,
    pub(crate) ast_id: AstId,
    // It's position in the Type array
    pub(crate) type_id: TypeId,
    pub(crate) fields: Vec<FieldRepre>,
    pub(crate) args: Vec<InnerArgs>,
    pub(crate) conds: Vec<Cond>,
}

impl StructRepre {
    pub(crate) fn new(
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
}

#[derive(Debug)]
pub(crate) struct EnumRepre {
    pub(crate) name_id: NameId,
    // Unsure about this positioning, I am hallucinating.
    pub(crate) sym_id: SymbolId,
    pub(crate) ast_id: AstId,
    pub(crate) type_id: TypeId,
    pub(crate) variants: Vec<VariantRepre>,
    pub(crate) args: Vec<InnerArgs>,
    pub(crate) conds: Vec<Cond>,
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
    pub(crate) name_id: NameId,
    // Because enum types are nullable
    pub(crate) type_id: Option<TypeId>,
    // Possible tuple
    // Points to variant within original Ast enum
    pub(crate) ast_id: AstId,
    pub(crate) args: Vec<InnerArgs>,
    pub(crate) conds: Vec<Cond>,
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
pub(crate) struct TypeDefRepre {
    pub(crate) name_id: NameId,
    pub(crate) sym_id: SymbolId,
    pub(crate) ast_id: AstId,
    pub(crate) type_id: TypeId,
    pub(crate) conds: Vec<Cond>,
    pub(crate) args: Vec<InnerArgs>,
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
    pub(crate) name_id: NameId,
    // Type reference to it's own position in the `Type` array
    pub(crate) type_id: TypeId,
    pub(crate) call_span: Span,
    pub(crate) kind: FuncKind,
    pub(crate) constraints: Vec<ArgConstraint>,
    pub(crate) args: Vec<FuncArgsRepre>,
}

impl FuncRepre {
    pub(crate) fn new(
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
    Integer(ValueId),
    Float(ValueId),
    Char(char),
    //TEST:
    Var(TypeId, BuiltinTypeKind),
    Default(TypeId, BuiltinTypeKind, ValueId),
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
pub(crate) struct FieldRepre {
    pub(crate) name_id: NameId,
    pub(crate) type_id: TypeId,
    // Ast contained field id, maybe this should just be AstId
    pub(crate) ast_id: AstId,
}

impl FieldRepre {
    pub(crate) fn new(name_id: NameId, type_id: TypeId, ast_id: AstId) -> FieldRepre {
        FieldRepre {
            name_id,
            type_id,
            ast_id,
        }
    }
}

#[derive(Debug)]
pub(crate) struct AliasRepre {
    pub(crate) name_id: NameId,
    pub(crate) sym_id: SymbolId,
    pub(crate) ast_id: AstId,
    // Refers to self's type id position
    // Maybe call this type_addr?
    pub(crate) type_id: TypeId,
    pub(crate) params: Vec<TypeId>,
    pub(crate) conds: Vec<Cond>,
    pub(crate) args: Vec<InnerArgs>,
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
pub struct Tuple {
    pub(crate) elements: Vec<TypeId>,
    pub(crate) type_id: TypeId,
}

impl Tuple {
    pub fn new(elements: Vec<TypeId>, type_id: TypeId) -> Tuple {
        Tuple { elements, type_id }
    }
}
