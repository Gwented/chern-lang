// TypeId is the index of the type itself, OR the type it's pointing to
use std::{collections::HashMap, fmt::Display};

use chrn_utils::{
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
    semantic::{constraints::ArgConstraint, scopes::ScopeType},
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

//NOTE: Should be in chrn_utils?
#[derive(Debug)]
pub enum Type {
    BuiltinType(BuiltinType),
    Struct(StructDef),
    Enum(EnumDef),
    Func(FuncDef),
    Alias(AliasDef),
    TypeDef(TypeDef),
    Unknown,
}

// Iyad yourrg gieyetters iiyand sieyetters
#[derive(Debug)]
pub struct Symbol {
    pub name_id: InternedId,
    pub sym_id: SymbolId,
    //err span purposes
    pub ast_id: AstId,
    pub kind: SymbolKind,
    pub owner: ModuleId,
    pub scope_type: ScopeType,
    pub is_priv: bool,
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
        scope_type: ScopeType,
        kind: SymbolKind,
    ) -> Symbol {
        Symbol {
            name_id,
            sym_id,
            ast_id,
            kind,
            scope_type,
            owner,
            is_priv,
        }
    }
}

/// Maps to a `TypeId`, `ValueId`, or `Unknown`
#[derive(Debug, Clone, Copy)]
pub enum SymbolKind {
    Type(TypeId),
    Val(ValueId),
    Unknown,
}

#[derive(Debug)]
pub struct Param {
    pub name_id: InternedId,
    //FIX: More like "FieldId"
    pub ast_id: AstId,
    pub type_id: TypeId,
}

impl Param {
    pub fn new(name_id: InternedId, ast_id: AstId, type_id: TypeId) -> Param {
        Param {
            name_id,
            type_id,
            ast_id,
        }
    }
}

#[derive(Debug)]
pub struct ResolvedExpr {
    // NOTE: Considering making a typesafe wrapper to unknown check explicitly
    pub type_id: TypeId,
    pub expr_hir: ExprHir,
    // May store these elsewhere depending on um...uh..unreachable!()
    pub inputs: Vec<ExprId>,
    // Should be one
    pub user: Option<ExprId>,
    pub span: Span,
    // This is not an option type even though `Value` as an `Option<Value>` because symbols are
    // already represented as unknown from `SymbolKind` and `Value` types already have the metadata
    // of their type and if they have a const value inside.
    pub val_id: ValueId,
}

impl ResolvedExpr {
    pub fn new(
        type_id: TypeId,
        expr_hir: ExprHir,
        val_id: ValueId,
        span: Span,
        inputs: Vec<ExprId>,
    ) -> ResolvedExpr {
        ResolvedExpr {
            type_id,
            expr_hir,
            inputs,
            span,
            user: None,
            val_id,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum ExprHir {
    Val(ValueId),
    Var(SymbolId),
    /// alias default(x) = [Equals(x = 3)]
    /// x = `SymbolId`, 5 = `ExprId`
    Default(SymbolId, ExprId),
    // Call(),
    // MemberAccess(),
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
pub struct StructDef {
    pub sym_id: SymbolId,
    pub fields: Vec<FieldRepre>,
    pub glob_conds: Vec<ExprId>,
    //Maybe SpannedInnerArgs is fine here
    pub glob_args: Vec<InnerArgs>,
}

impl StructDef {
    pub fn new(sym_id: SymbolId, fields: Vec<FieldRepre>) -> StructDef {
        StructDef {
            sym_id,
            fields,
            glob_conds: Vec::new(),
            glob_args: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct EnumDef {
    pub sym_id: SymbolId,
    pub variants: Vec<VariantRepre>,
    pub args: Vec<InnerArgs>,
    pub glob_conds: Vec<ExprId>,
}

impl EnumDef {
    pub fn new(sym_id: SymbolId, variants: Vec<VariantRepre>) -> EnumDef {
        EnumDef {
            sym_id,
            variants,
            glob_conds: Vec::new(),
            args: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct VariantRepre {
    pub name_id: InternedId,
    // Because enum types are nullable
    pub type_id: Option<TypeId>,
    // Points to variant within original Ast enum
    pub ast_id: AstId,
    pub args: Vec<InnerArgs>,
    pub conds: Vec<ExprId>,
}

impl VariantRepre {
    pub fn new(name_id: InternedId, type_id: Option<TypeId>, ast_id: AstId) -> VariantRepre {
        VariantRepre {
            name_id,
            type_id,
            ast_id,
            conds: Vec::new(),
            args: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct TypeDef {
    // Typedefs are: "var-> name: str" meaning the typedef type has types so it has a type id
    sym_id: SymbolId,
    pub type_id: TypeId,
    pub conds: Vec<ExprId>,
    pub args: Vec<InnerArgs>,
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
pub struct FuncDef {
    pub call_span: Span,
    pub kind: FuncKind,
    pub constraints: Vec<ArgConstraint>,
    pub args: Vec<FuncArgsRepre>,
}

impl FuncDef {
    pub fn new(
        call_span: Span,
        kind: FuncKind,
        constraints: Vec<ArgConstraint>,
        args: Vec<FuncArgsRepre>,
    ) -> FuncDef {
        FuncDef {
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
pub enum FuncArgsRepre {
    Integer(ValueId),
    Float(ValueId),
    Char(char),
    //TEST:
    Var(TypeId, BuiltinTypeKind),
    Default(TypeId, BuiltinTypeKind, ValueId),
    Str(InternedId),
}

impl FuncArgsRepre {
    pub fn is_numeric(&self) -> bool {
        match self {
            FuncArgsRepre::Integer(_) | FuncArgsRepre::Float(_) => true,
            FuncArgsRepre::Char(_) | FuncArgsRepre::Str(_) => false,
            FuncArgsRepre::Var(_, kind) | FuncArgsRepre::Default(_, kind, _) => kind.is_numeric(),
        }
    }

    // TEST: Currently testing out a way of formatting that is more encapsulated so that the code
    // outside doesn't have to be repeated as intensely.
    pub fn is_integer(&self) -> bool {
        match self.kind() {
            FuncArgsKind::Integer => true,
            _ => false,
        }
    }

    pub fn is_float(&self) -> bool {
        match self.kind() {
            FuncArgsKind::Float => true,
            _ => false,
        }
    }

    pub fn is_char(&self) -> bool {
        match self.kind() {
            FuncArgsKind::Char => true,
            _ => false,
        }
    }

    pub fn is_str(&self) -> bool {
        match self.kind() {
            FuncArgsKind::Str => true,
            _ => false,
        }
    }

    // to_builtin_type_kind is getting a little long for something so contextually obvious
    pub fn to_builtin_kind(&self) -> BuiltinTypeKind {
        match self {
            FuncArgsRepre::Integer(_) => BuiltinTypeKind::I64,
            FuncArgsRepre::Float(_) => BuiltinTypeKind::F64,
            FuncArgsRepre::Char(_) => BuiltinTypeKind::Char,
            FuncArgsRepre::Var(_, kind) | FuncArgsRepre::Default(_, kind, _) => *kind,
            FuncArgsRepre::Str(_) => BuiltinTypeKind::Str,
        }
    }

    pub fn kind(&self) -> FuncArgsKind {
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
pub enum FuncArgsKind {
    Integer,
    Float,
    Char,
    Var,
    Default,
    Str,
}

#[derive(Debug)]
pub struct FieldRepre {
    pub name_id: InternedId,
    // To TypeDef
    pub type_id: TypeId,
    // Ast contained field id, maybe this should just be AstId
    pub ast_id: AstId,
    pub conds: Vec<ExprId>,
    pub args: Vec<InnerArgs>,
}

impl FieldRepre {
    pub fn new(name_id: InternedId, type_id: TypeId, ast_id: AstId) -> FieldRepre {
        FieldRepre {
            name_id,
            type_id,
            conds: Vec::new(),
            args: Vec::new(),
            ast_id,
        }
    }
}

#[derive(Debug)]
pub struct AliasDef {
    pub sym_id: SymbolId,
    pub params: Vec<Param>,
    pub args: Vec<InnerArgs>,
    pub conds: Vec<ExprId>,
}

impl AliasDef {
    pub fn new(
        sym_id: SymbolId,
        params: Vec<Param>,
        conds: Vec<ExprId>,
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
