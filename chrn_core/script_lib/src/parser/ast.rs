use chrn_utils::{
    id_types::{AstId, InternedId},
    inner_args::SpannedInnerArgs,
    types::type_constraints::{self, TypeConstraintFlags},
};
//NOTE: MAY MAKE EXPRESSION THAT HELPS RESOLVE SEMANTIC TYPES MORE CLEANLY
use common::{
    fmter::{Formattable, Formatted},
    span::Span,
};

use crate::token::Notation;

#[derive(Debug)]
pub struct AstInfo {
    // Maybe eventually just use a 5 sized array since there are max 5 sections
    pub(crate) sections: [Option<Section>; 5],
    pub(crate) items: Vec<Item>,
}

impl AstInfo {
    pub(crate) fn new() -> AstInfo {
        AstInfo {
            sections: [None, None, None, None, None],
            items: Vec::new(),
        }
    }

    pub fn push_item(&mut self, kind: SectionKind, item: Item) {
        let ast_id = AstId::new(self.items.len() as u32);
        self.items.push(item);

        let sect = if let Some(sect) = &mut self.sections[kind as usize] {
            sect
        } else {
            self.push_sect(kind);
            &mut self.sections[kind as usize].as_mut().expect("Just created")
        };

        sect.push_ast_id(ast_id);
    }

    pub fn push_sect(&mut self, kind: SectionKind) {
        match kind {
            SectionKind::Neutral => {
                self.sections[kind as usize] = Some(Section::Neutral(Vec::new()));
            }
            SectionKind::Var => {
                self.sections[kind as usize] = Some(Section::Var(Vec::new()));
            }
            SectionKind::Nest => {
                self.sections[kind as usize] = Some(Section::Nest(Vec::new()));
            }
            SectionKind::Override => {
                self.sections[kind as usize] = Some(Section::Override(Vec::new()));
            }
            SectionKind::Complex => {
                self.sections[kind as usize] = Some(Section::Complex(Vec::new()));
            }
        }
    }

    pub fn sections(&self) -> &[Option<Section>] {
        &self.sections
    }

    pub fn items(&self) -> &Vec<Item> {
        &self.items
    }

    pub fn get_typedef(&self, ast_id: AstId) -> &AbstractTypeDef {
        match &self.items[ast_id.id as usize] {
            item => match item {
                Item::TypeDef(abs_typedef) => abs_typedef,
                _ => unreachable!(),
            },
        }
    }

    pub fn get_struct(&self, ast_id: AstId) -> &AbstractStruct {
        match &self.items[ast_id.id as usize] {
            item => match item {
                Item::Struct(abs_struct) => abs_struct,
                _ => unreachable!(),
            },
        }
    }

    pub fn get_const(&self, ast_id: AstId) -> &AbstractVar {
        match &self.items[ast_id.id as usize] {
            item => match item {
                Item::Var(abs_var) => abs_var,
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

    pub fn get_enum(&self, ast_id: AstId) -> &AbstractEnum {
        match &self.items[ast_id.id as usize] {
            item => match item {
                Item::Enum(abs_enum) => abs_enum,
                _ => unreachable!(),
            },
        }
    }

    pub fn get_alias(&self, ast_id: AstId) -> &AbstractAlias {
        match &self.items[ast_id.id as usize] {
            item => match item {
                Item::Alias(abs_alias) => abs_alias,
                _ => unreachable!(),
            },
        }
    }

    pub fn get_sym_span(&self, ast_id: AstId) -> Span {
        match &self.items[ast_id.id as usize] {
            Item::TypeDef(abs_typedef) => abs_typedef.name_span,
            Item::Struct(abs_struct) => abs_struct.name_span,
            Item::Enum(abs_enum) => abs_enum.name_span,
            Item::Alias(abs_alias) => abs_alias.name_span,
            Item::Var(abs_var) => abs_var.name_span,
        }
    }
}

#[derive(Debug)]
pub enum Item {
    //                                                 name: str [!IsEmpty, Range(0,5)]
    // Should these have spans? Do we REALLY want      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    TypeDef(AbstractTypeDef),
    Struct(AbstractStruct),
    Enum(AbstractEnum),
    Alias(AbstractAlias),
    Var(AbstractVar),
    // Func(AbstractFunc),
}

#[derive(Debug)]
pub enum Section {
    Neutral(Vec<AstId>),
    Var(Vec<AstId>),
    Nest(Vec<AstId>),
    Override(Vec<AstId>),
    Complex(Vec<AstId>),
}

impl Section {
    fn push_ast_id(&mut self, ast_id: AstId) {
        match self {
            Section::Neutral(ast_ids)
            | Section::Var(ast_ids)
            | Section::Nest(ast_ids)
            | Section::Override(ast_ids)
            | Section::Complex(ast_ids) => ast_ids.push(ast_id),
        }
    }

    pub fn kind(&self) -> SectionKind {
        match self {
            Section::Neutral(_) => SectionKind::Neutral,
            Section::Var(_) => SectionKind::Var,
            Section::Nest(_) => SectionKind::Nest,
            Section::Override(_) => SectionKind::Nest,
            Section::Complex(_) => SectionKind::Complex,
        }
    }
}

#[derive(Clone, Copy)]
#[repr(u32)]
pub enum SectionKind {
    Neutral,
    Var,
    Nest,
    Override,
    Complex,
}

#[derive(Debug)]
pub struct SpannedExpr {
    pub expr: Expr,
    pub span: Span,
}

impl SpannedExpr {
    pub fn new(expr: Expr, span: Span) -> SpannedExpr {
        SpannedExpr { expr, span }
    }
}

// This could look better...
// Does this need a literal specific variant?
#[derive(Debug)]
pub enum Expr {
    Var(InternedId),
    /// \`::\`
    StaticAccess(Vec<SpannedPathSegment>),
    Bool(bool),
    /// Variable name, along with optional default type
    Default(Box<SpannedExpr>, Box<SpannedExpr>),
    Integer(u32, Notation),
    Float(u32, Notation),
    Str(InternedId),
    Char(char),
    /// Caller, Args
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
            | BinaryOp::BitOr
            | BinaryOp::BitAnd
            | BinaryOp::BitNot
            | BinaryOp::BitRightShift
            | BinaryOp::BitLeftShift
            | BinaryOp::BitXor
            | BinaryOp::Mod => true,
            BinaryOp::Less
            | BinaryOp::GreaterOrEq
            | BinaryOp::Greater
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
            | BinaryOp::NotEq => true,
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mult
            | BinaryOp::Div
            | BinaryOp::BitOr
            | BinaryOp::BitAnd
            | BinaryOp::BitNot
            | BinaryOp::BitRightShift
            | BinaryOp::BitLeftShift
            | BinaryOp::BitXor
            | BinaryOp::Greater
            | BinaryOp::Mod => false,
        }
    }
}

impl Formattable for BinaryOp {
    fn to_fmt(&self) -> common::fmter::Formatted {
        match self {
            BinaryOp::Add => Formatted::OpAdd,
            BinaryOp::Sub => Formatted::Hyphen,
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

#[derive(Debug, Clone)]
pub struct SpannedTypeExpr {
    pub ty_expr: TypeExpr,
    pub span: Span,
}

impl SpannedTypeExpr {
    pub fn new(ty_expr: TypeExpr, span: Span) -> SpannedTypeExpr {
        SpannedTypeExpr { ty_expr, span }
    }
}

#[derive(Debug, Clone)]
pub enum TypeExpr {
    Var(InternedId),
    Path(Vec<SpannedPathSegment>),
    Generic(Generic),
}

#[derive(Debug, Clone)]
pub struct SpannedPathSegment {
    pub kind: PathSegment,
    pub span: Span,
}

impl SpannedPathSegment {
    pub fn new(kind: PathSegment, span: Span) -> Self {
        SpannedPathSegment { kind, span }
    }
}

#[derive(Debug, Clone)]
pub enum PathSegment {
    Ident(InternedId),
    Generic(Generic),
}

// Maybe type inference could pick up on the fact that if a definition has a condition, and that
// condition is applied to only a particular bit size, then it should be that bit size
#[derive(Debug)]
pub struct AbstractVar {
    pub name_id: InternedId,
    pub name_span: Span,
    pub spanned_expr: SpannedExpr,
    pub is_priv: bool,
}

impl AbstractVar {
    pub fn new(
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
pub struct AbstractTypeDef {
    pub name_id: InternedId,
    pub name_span: Span,
    pub spanned_ty_expr: SpannedTypeExpr,
    pub conds: Vec<SpannedExpr>,
    pub args: Vec<SpannedInnerArgs>,
}

impl AbstractTypeDef {
    pub fn new(
        name_id: InternedId,
        name_span: Span,
        spanned_ty_expr: SpannedTypeExpr,
        args: Vec<SpannedInnerArgs>,
        conds: Vec<SpannedExpr>,
    ) -> AbstractTypeDef {
        AbstractTypeDef {
            name_id,
            name_span,
            spanned_ty_expr,
            args,
            conds,
        }
    }
}

#[derive(Debug)]
pub struct AbstractStruct {
    pub name_id: InternedId,
    pub name_span: Span,
    pub glob_conds: Vec<SpannedExpr>,
    pub glob_args: Vec<SpannedInnerArgs>,
    pub fields: Vec<AbstractTypeDef>,
    pub is_priv: bool,
}

impl AbstractStruct {
    pub fn new(
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
pub struct AbstractEnum {
    // Would be SymbolId in symbol table anyways
    pub name_id: InternedId,
    pub name_span: Span,
    pub variants: Vec<AbstractVariant>,
    pub glob_conds: Vec<SpannedExpr>,
    pub glob_args: Vec<SpannedInnerArgs>,
    pub is_priv: bool,
    // pub(crate) visibility: Visibility,
}

impl AbstractEnum {
    pub fn new(
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
pub struct AbstractVariant {
    pub name_id: InternedId,
    pub name_span: Span,
    // I think this is right?
    pub ty_expr: Option<SpannedTypeExpr>,
    pub args: Vec<SpannedInnerArgs>,
    pub conds: Vec<SpannedExpr>,
}

impl AbstractVariant {
    pub fn new(
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
pub struct AbstractFunc {
    pub name_id: InternedId,
    pub name_span: Span,
    pub params: Vec<SpannedExpr>,
}

impl AbstractFunc {
    pub fn new(name_id: InternedId, name_span: Span, params: Vec<SpannedExpr>) -> AbstractFunc {
        AbstractFunc {
            name_id,
            name_span,
            params,
        }
    }
}

#[derive(Debug)]
pub struct AbstractFuncDecl {
    pub name_id: InternedId,
    pub name_span: Span,
    pub params: Vec<AbstractParam>,
}

impl AbstractFuncDecl {
    pub fn new(
        name_id: InternedId,
        name_span: Span,
        params: Vec<AbstractParam>,
    ) -> AbstractFuncDecl {
        AbstractFuncDecl {
            name_id,
            name_span,
            params,
        }
    }
}

#[derive(Debug)]
pub struct AbstractParam {
    pub name_id: InternedId,
    pub name_span: Span,
    pub ty_expr: SpannedTypeExpr,
}

impl AbstractParam {
    pub fn new(name_id: InternedId, name_span: Span, ty_expr: SpannedTypeExpr) -> AbstractParam {
        AbstractParam {
            name_id,
            name_span,
            ty_expr,
        }
    }
}

#[derive(Debug)]
pub struct AbstractFieldDecl {
    pub name_id: InternedId,
    pub fields: Vec<SpannedExpr>,
}

// impl AbstractFieldDecl {
//     pub(crate) fn new(base: Box<Expr>, fields: Vec<Expr>) -> AbstractFieldDecl {
//         AbstractFieldDecl { base, fields }
//     }
// }

#[derive(Debug)]
pub struct AbstractAlias {
    pub name_id: InternedId,
    pub name_span: Span,
    // Variables only
    // May change to Vec<SpannedInternedId>
    pub params: Vec<AbstractParam>,
    pub conds: Vec<SpannedExpr>,
    pub args: Vec<SpannedInnerArgs>,
    pub is_priv: bool,
}

impl AbstractAlias {
    pub fn new(
        name_id: InternedId,
        name_span: Span,
        params: Vec<AbstractParam>,
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
pub struct AbstractMemberAccess {
    pub base: Box<SpannedExpr>,
    pub field: InternedId,
}

impl AbstractMemberAccess {
    pub fn new(base: Box<SpannedExpr>, field: InternedId) -> AbstractMemberAccess {
        AbstractMemberAccess { base, field }
    }
}

#[derive(Debug)]
pub struct Unary {
    pub op: UnaryOp,
    pub spanned_expr: Box<SpannedExpr>,
}

impl Unary {
    pub fn new(op: UnaryOp, spanned_expr: Box<SpannedExpr>) -> Unary {
        Unary { op, spanned_expr }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum UnaryOp {
    Not,
    Negate,
}

impl UnaryOp {
    pub fn type_constraints(&self) -> TypeConstraintFlags {
        let flags = match self {
            UnaryOp::Not => type_constraints::BOOL,
            UnaryOp::Negate => type_constraints::NUMERIC,
        };

        TypeConstraintFlags::new(flags)
    }
}

impl Formattable for UnaryOp {
    fn to_fmt(&self) -> Formatted {
        match self {
            UnaryOp::Not => Formatted::ExclamationPoint,
            UnaryOp::Negate => Formatted::Hyphen,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Generic {
    pub base: InternedId,
    // Change to tuple or something alike since max 2?
    pub args: Vec<SpannedTypeExpr>,
}

impl Generic {
    pub fn new(base: InternedId, args: Vec<SpannedTypeExpr>) -> Generic {
        Generic { base, args }
    }
}
