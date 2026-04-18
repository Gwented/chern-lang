use chern_core::{
    id_types::{AstId, InternedId, PathId},
    inner_args::SpannedInnerArgs,
};
//NOTE: MAY MAKE EXPRESSION THAT HELPS RESOLVE SEMANTIC TYPES MORE CLEANLY
use common::{
    fmter::{Formattable, Formatted},
    span::Span,
};

use crate::token::Notation;

#[derive(Debug)]
pub struct AstInfo {
    pub(crate) items: Vec<Item>,
}

impl AstInfo {
    pub(crate) fn new() -> AstInfo {
        AstInfo { items: Vec::new() }
    }

    pub(crate) fn get_typedef(&self, ast_id: AstId) -> &AbstractTypeDef {
        match &self.items[ast_id.id as usize] {
            item => match item {
                Item::TypeDef(abs_typedef) => abs_typedef,
                _ => unreachable!(),
            },
        }
    }

    pub(crate) fn get_struct(&self, ast_id: AstId) -> &AbstractStruct {
        match &self.items[ast_id.id as usize] {
            item => match item {
                Item::Struct(abs_struct) => abs_struct,
                _ => unreachable!(),
            },
        }
    }

    pub(crate) fn get_const(&self, ast_id: AstId) -> &AbstractVar {
        match &self.items[ast_id.id as usize] {
            item => match item {
                Item::VarDecl(abs_var) => abs_var,
                _ => unreachable!(),
            },
        }
    }

    // pub(super) fn get_func(&self, ast_id: AstId) -> &AbstractFunc {
    //     match &self.items[ast_id.id as usize] {
    //         item => match item {
    //             Item::Func(abs_struct) => abs_struct,
    //             _ => unreachable!(),
    //         },
    //     }
    // }

    pub(crate) fn get_enum(&self, ast_id: AstId) -> &AbstractEnum {
        match &self.items[ast_id.id as usize] {
            item => match item {
                Item::Enum(abs_enum) => abs_enum,
                _ => unreachable!(),
            },
        }
    }

    pub(crate) fn get_ast_span(&self, ast_id: AstId) -> Span {
        match &self.items[ast_id.id as usize] {
            Item::TypeDef(abs_typedef) => abs_typedef.name_span,
            Item::Struct(abs_struct) => abs_struct.name_span,
            Item::Enum(abs_enum) => abs_enum.name_span,
            Item::Alias(abs_alias) => abs_alias.name_span,
            Item::VarDecl(abs_var) => abs_var.name_span,
        }
    }
}

#[derive(Debug)]
pub(crate) enum Item {
    //                                                 name: str [!IsEmpty, Range(0,5)]
    // Should these have spans? Do we REALLY want      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    TypeDef(AbstractTypeDef),
    Struct(AbstractStruct),
    Enum(AbstractEnum),
    Alias(AbstractAlias),
    VarDecl(AbstractVar),
    // Func(AbstractFunc),
}

//TEST:
// #[derive(Debug)]
// pub(crate) enum Section {
//     Var(Vec<Item>),
//     Nest(Vec<Item>),
//     Override(Vec<Item>),
//     Complex(Vec<Item>),
// }

#[derive(Debug)]
pub(crate) struct SpannedExpr {
    pub(crate) expr: Expr,
    pub(crate) span: Span,
}

impl SpannedExpr {
    pub fn new(expr: Expr, span: Span) -> SpannedExpr {
        SpannedExpr { expr, span }
    }
}

// This could look better...
// Does this need a literal specific variant?
#[derive(Debug)]
pub(crate) enum Expr {
    Var(InternedId),
    Bool(bool),
    /// Variable name, along with optional default type
    Default(InternedId, Box<SpannedExpr>),
    // TODO: Need to likely carry notation here
    // Also maybe should be a "literal" type
    Integer(u32, Notation),
    Float(u32, Notation),
    Str(InternedId),
    Char(char),
    Call(Box<SpannedExpr>, Vec<SpannedExpr>),
    MemberAccess(AbstractMemberAccess),
    Unary(Unary),
    BinaryExpr {
        lhs: Box<SpannedExpr>,
        op: BinaryOp,
        rhs: Box<SpannedExpr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mult,
    Div,
    Greater,
    Less,
    GreaterOrEq,
    LessOrEq,
    Mod,
    And,
    Or,
    EqTo,
    NotEq,
    BitOr,
    BitAnd,
    BitNot,
    BitRightShift,
    BitLeftShift,
    BitXor,
}

impl BinaryOp {
    pub fn is_arithmetic_op(&self) -> bool {
        match self {
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mult
            | BinaryOp::Div
            | BinaryOp::Greater
            | BinaryOp::BitOr
            | BinaryOp::BitAnd
            | BinaryOp::BitNot
            | BinaryOp::BitRightShift
            | BinaryOp::BitLeftShift
            | BinaryOp::BitXor
            | BinaryOp::Mod => true,
            BinaryOp::Less
            | BinaryOp::GreaterOrEq
            | BinaryOp::LessOrEq
            | BinaryOp::And
            | BinaryOp::Or
            | BinaryOp::EqTo
            | BinaryOp::NotEq => false,
        }
    }

    pub fn is_bool_op(&self) -> bool {
        match self {
            BinaryOp::Less
            | BinaryOp::GreaterOrEq
            | BinaryOp::LessOrEq
            | BinaryOp::And
            | BinaryOp::Or
            | BinaryOp::EqTo
            | BinaryOp::BitOr
            | BinaryOp::BitAnd
            | BinaryOp::BitNot
            | BinaryOp::BitRightShift
            | BinaryOp::BitLeftShift
            | BinaryOp::BitXor
            | BinaryOp::NotEq => true,
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mult
            | BinaryOp::Div
            | BinaryOp::Greater
            | BinaryOp::Mod => false,
        }
    }
}

impl Formattable for BinaryOp {
    fn to_fmt(&self) -> common::fmter::Formatted {
        match self {
            BinaryOp::Add => Formatted::OpAdd,
            BinaryOp::Sub => Formatted::OpSub,
            BinaryOp::Mult => Formatted::OpMult,
            BinaryOp::Div => Formatted::OpDivide,
            BinaryOp::Greater => Formatted::OpGreater,
            BinaryOp::Less => Formatted::OpLess,
            BinaryOp::GreaterOrEq => Formatted::OpGreaterOrEq,
            BinaryOp::LessOrEq => Formatted::OpLessOrEq,
            BinaryOp::Mod => Formatted::OpMod,
            BinaryOp::And => Formatted::OpAnd,
            BinaryOp::Or => Formatted::OpOr,
            BinaryOp::EqTo => Formatted::OpEqualTo,
            BinaryOp::NotEq => Formatted::OpNotEq,
            BinaryOp::BitOr => Formatted::OpBitOr,
            BinaryOp::BitAnd => Formatted::OpBitAnd,
            BinaryOp::BitNot => Formatted::OpBitNot,
            BinaryOp::BitRightShift => Formatted::OpBitRightShift,
            BinaryOp::BitLeftShift => Formatted::OpBitLeftShift,
            BinaryOp::BitXor => Formatted::OpBitXor,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Call {
    pub(crate) name_id: InternedId,
    pub(crate) spanned_expr: Vec<SpannedExpr>,
}

impl Call {
    pub(crate) fn new(name_id: InternedId, spanned_expr: Vec<SpannedExpr>) -> Call {
        Call {
            name_id,
            spanned_expr,
        }
    }
}

#[derive(Debug)]
pub(crate) struct SpannedTypeExpr {
    pub(crate) ty_expr: TypeExpr,
    pub(crate) span: Span,
}

impl SpannedTypeExpr {
    pub fn new(ty_expr: TypeExpr, span: Span) -> SpannedTypeExpr {
        SpannedTypeExpr { ty_expr, span }
    }
}

#[derive(Debug)]
pub(crate) enum TypeExpr {
    Var(InternedId),
    Path(Vec<SpannedTypeExpr>),
    Escaped(InternedId),
    Generic(Generic),
    Tuple(Vec<SpannedTypeExpr>),
    Any,
}

// Maybe type inference could pick up on the fact that if a definition has a condition, and that
// condition is applied to only a particular bit size, then it should be that bit size
#[derive(Debug)]
pub(crate) struct AbstractVar {
    pub(crate) name_id: InternedId,
    pub(crate) name_span: Span,
    pub(crate) spanned_expr: SpannedExpr,
    pub(crate) is_priv: bool,
}

impl AbstractVar {
    pub(crate) fn new(
        name_id: InternedId,
        name_span: Span,
        spanned_expr: SpannedExpr,
        is_priv: bool,
    ) -> AbstractVar {
        AbstractVar {
            name_id,
            name_span,
            spanned_expr,
            is_priv,
        }
    }
}

#[derive(Debug)]
pub(crate) struct AbstractTypeDef {
    pub(crate) name_id: InternedId,
    pub(crate) name_span: Span,
    pub(crate) spanned_ty_expr: SpannedTypeExpr,
    pub(crate) args: Vec<SpannedInnerArgs>,
    pub(crate) conds: Vec<SpannedExpr>,
}

impl AbstractTypeDef {
    pub(crate) fn new(
        name_id: InternedId,
        name_span: Span,
        ty_expr: SpannedTypeExpr,
        args: Vec<SpannedInnerArgs>,
        conds: Vec<SpannedExpr>,
    ) -> AbstractTypeDef {
        AbstractTypeDef {
            name_id,
            name_span,
            spanned_ty_expr: ty_expr,
            args,
            conds,
        }
    }
}

#[derive(Debug)]
pub(crate) struct AbstractStruct {
    pub(crate) name_id: InternedId,
    pub(crate) name_span: Span,
    pub(crate) glob_conds: Vec<SpannedExpr>,
    pub(crate) glob_args: Vec<SpannedInnerArgs>,
    pub(crate) fields: Vec<AbstractTypeDef>,
    pub(crate) is_priv: bool,
}

impl AbstractStruct {
    pub(crate) fn new(
        name_id: InternedId,
        name_span: Span,
        glob_conds: Vec<SpannedExpr>,
        glob_args: Vec<SpannedInnerArgs>,
        fields: Vec<AbstractTypeDef>,
        is_priv: bool,
        // visibility: Visibility,
    ) -> AbstractStruct {
        AbstractStruct {
            name_id,
            name_span,
            glob_args,
            glob_conds,
            fields,
            is_priv, // visibility,
        }
    }
}

#[derive(Debug)]
pub(crate) struct AbstractEnum {
    // Would be SymbolId in symbol table anyways
    pub(crate) name_id: InternedId,
    pub(crate) name_span: Span,
    pub(crate) variants: Vec<AbstractVariant>,
    pub(crate) glob_conds: Vec<SpannedExpr>,
    pub(crate) glob_args: Vec<SpannedInnerArgs>,
    pub(crate) is_priv: bool,
    // pub(crate) visibility: Visibility,
}

impl AbstractEnum {
    pub(crate) fn new(
        name_id: InternedId,
        name_span: Span,
        variants: Vec<AbstractVariant>,
        glob_conds: Vec<SpannedExpr>,
        glob_args: Vec<SpannedInnerArgs>,
        is_priv: bool,
    ) -> AbstractEnum {
        AbstractEnum {
            name_id,
            name_span,
            variants,
            glob_conds,
            glob_args,
            is_priv,
        }
    }
}

// Hold that thought
#[derive(Debug)]
pub(crate) struct AbstractVariant {
    pub(crate) name_id: InternedId,
    pub(crate) name_span: Span,
    // I think this is right?
    pub(crate) ty_expr: Option<SpannedTypeExpr>,
    pub(crate) args: Vec<SpannedInnerArgs>,
    pub(crate) conds: Vec<SpannedExpr>,
}

impl AbstractVariant {
    pub(crate) fn new(
        name_id: InternedId,
        name_span: Span,
        // I think this is right?
        ty_expr: Option<SpannedTypeExpr>,
        conds: Vec<SpannedExpr>,
        args: Vec<SpannedInnerArgs>,
    ) -> AbstractVariant {
        AbstractVariant {
            name_id,
            name_span,
            ty_expr,
            args,
            conds,
        }
    }
}

#[derive(Debug)]
pub(crate) struct AbstractFunc {
    pub(crate) name_id: InternedId,
    pub(crate) name_span: Span,
    pub(crate) params: Vec<SpannedExpr>,
}

impl AbstractFunc {
    pub(crate) fn new(
        name_id: InternedId,
        name_span: Span,
        params: Vec<SpannedExpr>,
    ) -> AbstractFunc {
        AbstractFunc {
            name_id,
            name_span,
            params,
        }
    }
}

#[derive(Debug)]
pub(crate) struct AbstractFuncDecl {
    pub(crate) name_id: InternedId,
    pub(crate) name_span: Span,
    pub(crate) params: Vec<Param>,
}

impl AbstractFuncDecl {
    pub(crate) fn new(
        name_id: InternedId,
        name_span: Span,
        params: Vec<Param>,
    ) -> AbstractFuncDecl {
        AbstractFuncDecl {
            name_id,
            name_span,
            params,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Param {
    pub(crate) name_id: InternedId,
    pub(crate) name_span: Span,
    pub(crate) ty_expr: SpannedTypeExpr,
}

impl Param {
    pub(crate) fn new(name_id: InternedId, name_span: Span, ty_expr: SpannedTypeExpr) -> Param {
        Param {
            name_id,
            name_span,
            ty_expr,
        }
    }
}

//TODO:
#[derive(Debug)]
pub(crate) struct AbstractFieldDecl {
    pub(crate) name_id: InternedId,
    pub(crate) fields: Vec<SpannedExpr>,
}

// impl AbstractFieldDecl {
//     pub(crate) fn new(base: Box<Expr>, fields: Vec<Expr>) -> AbstractFieldDecl {
//         AbstractFieldDecl { base, fields }
//     }
// }

#[derive(Debug)]
pub(crate) struct AbstractAlias {
    pub(crate) name_id: InternedId,
    pub(crate) name_span: Span,
    // Variables only
    // May change to param
    pub(crate) params: Vec<SpannedTypeExpr>,
    pub(crate) conds: Vec<SpannedExpr>,
    pub(crate) args: Vec<SpannedInnerArgs>,
    pub(crate) is_priv: bool,
}

impl AbstractAlias {
    pub(crate) fn new(
        name_id: InternedId,
        name_span: Span,
        params: Vec<SpannedTypeExpr>,
        conds: Vec<SpannedExpr>,
        args: Vec<SpannedInnerArgs>,
        is_priv: bool,
    ) -> AbstractAlias {
        AbstractAlias {
            name_id,
            name_span,
            params,
            conds,
            args,
            is_priv,
        }
    }
}

#[derive(Debug)]
pub(crate) struct AbstractMemberAccess {
    pub(crate) base: Box<SpannedExpr>,
    pub(crate) field: InternedId,
}

impl AbstractMemberAccess {
    pub(crate) fn new(base: Box<SpannedExpr>, field: InternedId) -> AbstractMemberAccess {
        AbstractMemberAccess { base, field }
    }
}

#[derive(Debug)]
pub(crate) struct Unary {
    pub(crate) op: UnaryOp,
    pub(crate) spanned_expr: Box<SpannedExpr>,
}

impl Unary {
    pub(crate) fn new(op: UnaryOp, expr: Box<SpannedExpr>) -> Unary {
        Unary {
            op,
            spanned_expr: expr,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum UnaryOp {
    Not,
    Negate,
}

impl Formattable for UnaryOp {
    fn to_fmt(&self) -> Formatted {
        match self {
            UnaryOp::Not => Formatted::ExclamationPoint,
            UnaryOp::Negate => Formatted::OpSub,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Generic {
    pub(crate) base: InternedId,
    // Change to tuple or something alike since max 2?
    pub(crate) args: Vec<SpannedTypeExpr>,
}

impl Generic {
    pub(crate) fn new(base: InternedId, args: Vec<SpannedTypeExpr>) -> Generic {
        Generic { base, args }
    }
}
