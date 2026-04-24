// TypeId is the index of the type itself, OR the type it's pointing to
use std::{collections::HashMap, fmt::Display};

use chern_core::{
    builtins::{BuiltinType, BuiltinTypeKind},
    id_types::{AstId, ExprId, InternedId, ModuleId, SymbolId, TypeId, ValueId},
    inner_args::InnerArgs,
};
use common::{
    fmter::{Formattable, Formatted},
    span::Span,
};

use crate::{
    conditions::Cond,
    parser::ast::{BinaryOp, UnaryOp},
    semantic::constraints::ArgConstraint,
};

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
//
// #[derive(Debug)]
// pub struct SymbolInfo {
//     pub symbol: Symbol,
//     pub owner: ModuleId,
//     pub is_priv: bool,
// }
//
// impl SymbolInfo {
//     pub fn new(symbol: Symbol, is_priv: bool, owner: ModuleId) -> SymbolInfo {
//         SymbolInfo {
//             symbol,
//             is_priv,
//             owner,
//         }
//     }
// }

//NOTE: Should be in chern_core?
#[derive(Debug)]
pub enum Type {
    BuiltinType(BuiltinType),
    Struct(StructDef),
    Enum(EnumDef),
    Func(FuncRepre),
    Alias(AliasDef),
    TypeDef(TypeDef),
    // Var(VarDef),
    Tuple(Tuple),
    Unknown,
}

#[derive(Debug)]
pub struct Symbol {
    pub(crate) name_id: InternedId,
    pub(crate) sym_id: SymbolId,
    //dbg purposes
    pub(crate) ast_id: AstId,
    //dbgr
    pub(crate) kind: SymbolKind,
    pub(crate) owner: ModuleId,
    pub(crate) is_priv: bool,
}

impl Symbol {
    pub fn new(
        // May couple dbg info but fine for now
        name_id: InternedId,
        sym_id: SymbolId,
        //dbgr
        ast_id: AstId,
        owner: ModuleId,
        is_priv: bool,
        sym_kind: SymbolKind,
    ) -> Symbol {
        Symbol {
            name_id,
            sym_id,
            ast_id,
            kind: sym_kind,
            owner,
            is_priv,
        }
    }
}

// May call this id kind..
/// Maps to a `TypeId` or `ValueId`
#[derive(Debug, Clone, Copy)]
pub(crate) enum SymbolKind {
    Type(TypeId),
    Val(ValueId),
    Unknown,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedExpr {
    pub(crate) type_id: TypeId,
    pub(crate) expr_hir: ExprHir,
    pub(crate) const_val: Option<ValueId>,
}

impl ResolvedExpr {
    pub fn new(type_id: TypeId, expr_hir: ExprHir, const_val: Option<ValueId>) -> ResolvedExpr {
        ResolvedExpr {
            type_id,
            expr_hir,
            const_val,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum ExprHir {
    Val(ValueId),
    Var(SymbolId),
    Default(SymbolId, ExprId),
    // Um
    // Call(Box<SpannedExpr>, Vec<SpannedExpr>),
    // MemberAccess(AbstractMemberAccess),
    Unary {
        op: UnaryOp,
        operand: ExprId,
    },
    BinaryExpr {
        lhs: ExprId,
        op: BinaryOp,
        rhs: ExprId,
    },
}

// #[derive(Debug)]
// pub(crate) enum Symbol {
//     TypeDef(TypeDefRepre),
//     Struct(StructRepre),
//     Func(FuncRepre),
//     Enum(EnumRepre),
//     Alias(AliasRepre),
//     Var(VarRepre),
// }

// impl Symbol {
//     pub(crate) fn name_id(&self) -> InternedId {
//         match self {
//             Symbol::TypeDef(type_def_repre) => type_def_repre.name_id,
//             Symbol::Struct(struct_def) => struct_def.name_id,
//             Symbol::Func(func_repre) => func_repre.name_id,
//             Symbol::Enum(enum_def) => enum_def.name_id,
//             Symbol::Alias(alias_def) => alias_def.name_id,
//             Symbol::Var(var_repre) => var_repre.name_id,
//         }
//     }
//
//     pub(crate) fn type_id(&self) -> TypeId {
//         match self {
//             Symbol::TypeDef(type_def_repre) => type_def_repre.type_id,
//             Symbol::Struct(struct_def) => struct_def.type_id,
//             Symbol::Func(func_repre) => func_repre.type_id,
//             Symbol::Enum(enum_def) => enum_def.type_id,
//             Symbol::Alias(alias_def) => alias_def.type_id,
//             Symbol::Var(var_repre) => var_repre.type_id,
//         }
//     }
// }

#[derive(Debug)]
pub struct Table {
    // Type specific tables
    pub(crate) name_ids: HashMap<AstId, InternedId>,
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

// #[derive(Debug)]
// pub(crate) struct VarDef {
//     pub(crate) type_id: TypeId,
//     pub(crate) expr_id: ExprId,
//     pub(crate) const_val: Option<ValueId>,
// }
//
// impl VarDef {
//     pub fn new(type_id: TypeId, expr_id: ExprId, const_val: Option<ValueId>) -> VarDef {
//         VarDef {
//             type_id,
//             expr_id,
//             const_val,
//         }
//     }
// }

#[derive(Debug)]
pub(crate) struct StructDef {
    pub(crate) sym_id: SymbolId,
    pub(crate) fields: Vec<FieldRepre>,
    pub(crate) args: Vec<InnerArgs>,
    pub(crate) conds: Vec<Cond>,
}

impl StructDef {
    pub(crate) fn new(sym_id: SymbolId, fields: Vec<FieldRepre>) -> StructDef {
        StructDef {
            sym_id,
            fields,
            args: Vec::new(),
            conds: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct EnumDef {
    // pub(crate) name_id: InternedId,
    // // Unsure about this positioning, I am hallucinating.
    // pub(crate) ast_id: AstId,
    // pub(crate) type_id: TypeId,
    pub(crate) sym_id: SymbolId,
    pub(crate) variants: Vec<VariantRepre>,
    pub(crate) args: Vec<InnerArgs>,
    pub(crate) conds: Vec<Cond>,
}

impl EnumDef {
    pub fn new(sym_id: SymbolId, variants: Vec<VariantRepre>) -> EnumDef {
        EnumDef {
            sym_id,
            variants,
            args: Vec::new(),
            conds: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct VariantRepre {
    pub(crate) name_id: InternedId,
    // Because enum types are nullable
    pub(crate) type_id: Option<TypeId>,
    // Possible tuple
    // Points to variant within original Ast enum
    pub(crate) ast_id: AstId,
    pub(crate) args: Vec<InnerArgs>,
    pub(crate) conds: Vec<Cond>,
}

impl VariantRepre {
    pub fn new(name_id: InternedId, type_id: Option<TypeId>, ast_id: AstId) -> VariantRepre {
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
pub(crate) struct TypeDef {
    // Typedefs are: "var-> name: str" meaning the typedef type has types so it has a type id
    pub(crate) sym_id: SymbolId,
    pub(crate) type_id: TypeId,
    pub(crate) conds: Vec<Cond>,
    pub(crate) args: Vec<InnerArgs>,
}

impl TypeDef {
    pub fn new(sym_id: SymbolId, type_id: TypeId) -> TypeDef {
        TypeDef {
            sym_id,
            type_id,
            conds: Vec::new(),
            args: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct FuncRepre {
    pub(crate) call_span: Span,
    pub(crate) kind: FuncKind,
    pub(crate) constraints: Vec<ArgConstraint>,
    pub(crate) args: Vec<FuncArgsRepre>,
}

impl FuncRepre {
    pub(crate) fn new(
        call_span: Span,
        kind: FuncKind,
        constraints: Vec<ArgConstraint>,
        args: Vec<FuncArgsRepre>,
    ) -> FuncRepre {
        FuncRepre {
            kind,
            call_span,
            constraints,
            args,
        }
    }
}

// I'm scared of this
// This should be removed
// TODO:
#[derive(Debug)]
pub(crate) enum FuncArgsRepre {
    Integer(ValueId),
    Float(ValueId),
    Char(char),
    //TEST:
    Var(TypeId, BuiltinTypeKind),
    Default(TypeId, BuiltinTypeKind, ValueId),
    Str(InternedId),
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
    pub(crate) name_id: InternedId,
    pub(crate) type_id: TypeId,
    // Ast contained field id, maybe this should just be AstId
    pub(crate) ast_id: AstId,
}

impl FieldRepre {
    pub(crate) fn new(name_id: InternedId, type_id: TypeId, ast_id: AstId) -> FieldRepre {
        FieldRepre {
            name_id,
            type_id,
            ast_id,
        }
    }
}

#[derive(Debug)]
pub(crate) struct AliasDef {
    pub(crate) sym_id: SymbolId,
    pub(crate) params: Vec<TypeId>,
    pub(crate) conds: Vec<Cond>,
    pub(crate) args: Vec<InnerArgs>,
}

impl AliasDef {
    pub fn new(
        sym_id: SymbolId,
        params: Vec<TypeId>,
        conds: Vec<Cond>,
        args: Vec<InnerArgs>,
    ) -> AliasDef {
        AliasDef {
            sym_id,
            params,
            conds,
            args,
        }
    }
}

#[derive(Debug)]
pub struct Tuple {
    pub(crate) elements: Vec<TypeId>,
}

impl Tuple {
    pub fn new(elements: Vec<TypeId>) -> Tuple {
        Tuple { elements }
    }
}

//TEST:
pub(crate) enum PossibleMember {
    Module(ModuleId),
    Type(TypeId),
    Var(ValueId),
    Nothing,
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
