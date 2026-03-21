//NOTE: MAY MAKE EXPRESSION THAT HELPS RESOLVE SEMANTIC TYPES MORE CLEANLY
use common::symbols::{InnerArgs, NameId, Span, SpannedInnerArgs};

#[derive(Debug)]
pub struct AstInfo {
    // MAYBE SHOULDN'T BE A NAME ID I DONT KNOW
    pub(crate) bind: Option<NameId>,
    pub(crate) items: Vec<Item>,
}

impl AstInfo {
    pub fn new() -> AstInfo {
        AstInfo {
            bind: None,
            items: Vec::new(),
        }
    }

    pub(crate) fn set_bind(&mut self, bind: NameId) {
        self.bind = Some(bind);
    }

    pub(crate) fn has_bind(&self) -> bool {
        self.bind.is_some()
    }
}

#[derive(Debug)]
pub(crate) enum Item {
    //                                                 name: str [!IsEmpty, Range(0,5)]
    //TODO: Should these have spans? Do we REALLY want ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    //
    Var(AbstractTypeDef),
    Struct(AbstractStruct),
    Enum(AbstractEnum),
    Alias(AbstractAlias),
    // Func(AbstractFunc),
}

// This could look better...
#[derive(Debug)]
pub(crate) enum Expr {
    Var(NameId, Span),
    /// Variable name and span, along with optional default type
    Default((NameId, Span), Option<Box<Expr>>),
    // Staying capped at i64 and f64 for pacing purposes
    // TODO: Need to likely carry notation here
    // Also maybe should be a "literal" type
    Integer(i64, Span),
    Float(f64, Span),
    Str(NameId, Span),
    Char(char, Span),
    Call(Call, Span),
    FieldAccess(AbstractFieldAccess, Span),
    Unary(Unary, Span),
    BinaryExpr {
        lhs: Box<Expr>,
        op: BinaryOp,
        rhs: Box<Expr>,
    },
}

#[derive(Debug)]
pub(crate) enum BinaryOp {
    Add,
    Sub,
    Mult,
    Div,
    Mod,
}

#[derive(Debug)]
pub(crate) struct Call {
    pub(crate) name_id: NameId,
    pub(crate) exprs: Vec<Expr>,
}

impl Call {
    pub(crate) fn new(name_id: NameId, exprs: Vec<Expr>) -> Call {
        Call { name_id, exprs }
    }
}

#[derive(Debug)]
pub(crate) enum TypeExpr {
    Var(NameId, Span),
    Escaped(NameId, Span),
    //_Generic
    Generic(AbstractGeneric, Span),
    Tuple(Vec<TypeExpr>, Span),
    Any(Span),
}

//TEST: May just make a SpannedTypeExpr but this can work for now
impl TypeExpr {
    pub fn span(&self) -> Span {
        match self {
            TypeExpr::Var(_, span)
            | TypeExpr::Generic(_, span)
            | TypeExpr::Any(span)
            | TypeExpr::Tuple(_, span)
            | TypeExpr::Escaped(_, span) => span.clone(),
        }
    }
}

// Maybe put in enum exclusively if not needed outside

// #[derive(Debug)]
// pub struct AbstractBind {
//     pub(crate) name_id: NameId,
// }
//
// impl AbstractBind {
//     pub fn new(name_id: NameId) -> AbstractBind {
//         AbstractBind { name_id }
//     }
// }
//
#[derive(Debug)]
pub struct AbstractTypeDef {
    pub(crate) name_id: NameId,
    pub(crate) name_span: Span,
    pub(crate) ty: TypeExpr,
    pub(crate) args: Vec<SpannedInnerArgs>,
    pub(crate) conds: Vec<Expr>,
}

impl AbstractTypeDef {
    pub(crate) fn new(
        name_id: NameId,
        name_span: Span,
        ty: TypeExpr,
        args: Vec<SpannedInnerArgs>,
        conds: Vec<Expr>,
    ) -> AbstractTypeDef {
        AbstractTypeDef {
            name_id,
            name_span,
            ty,
            args,
            conds,
        }
    }
}

#[derive(Debug)]
pub struct AbstractStruct {
    pub(crate) name_id: NameId,
    pub(crate) name_span: Span,
    pub(crate) glob_conds: Vec<Expr>,
    pub(crate) glob_args: Vec<SpannedInnerArgs>,
    pub(crate) fields: Vec<AbstractTypeDef>,
}

impl AbstractStruct {
    pub(crate) fn new(
        name_id: NameId,
        name_span: Span,
        glob_conds: Vec<Expr>,
        glob_args: Vec<SpannedInnerArgs>,
        fields: Vec<AbstractTypeDef>,
    ) -> AbstractStruct {
        AbstractStruct {
            name_id,
            name_span,
            glob_args,
            glob_conds,
            fields,
        }
    }
}

#[derive(Debug)]
pub struct AbstractEnum {
    // Would be SymbolId in symbol table anyways
    pub(crate) name_id: NameId,
    pub(crate) name_span: Span,
    pub(crate) variants: Vec<AbstractVariant>,
    pub(crate) glob_conds: Vec<Expr>,
    pub(crate) glob_args: Vec<SpannedInnerArgs>,
}

impl AbstractEnum {
    pub(crate) fn new(
        name_id: NameId,
        name_span: Span,
        variants: Vec<AbstractVariant>,
        glob_conds: Vec<Expr>,
        glob_args: Vec<SpannedInnerArgs>,
    ) -> AbstractEnum {
        AbstractEnum {
            name_id,
            name_span,
            variants,
            glob_conds,
            glob_args,
        }
    }
}

// Hold that thought
#[derive(Debug)]
pub(crate) struct AbstractVariant {
    pub(crate) name_id: NameId,
    pub(crate) name_span: Span,
    // I think this is right?
    pub(crate) ty: Option<TypeExpr>,
    pub(crate) args: Vec<SpannedInnerArgs>,
    pub(crate) conds: Vec<Expr>,
}

impl AbstractVariant {
    pub(crate) fn new(
        name_id: NameId,
        name_span: Span,
        // I think this is right?
        ty: Option<TypeExpr>,
        conds: Vec<Expr>,
        args: Vec<SpannedInnerArgs>,
    ) -> AbstractVariant {
        AbstractVariant {
            name_id,
            name_span,
            ty,
            args,
            conds,
        }
    }
}

#[derive(Debug)]
pub(crate) struct AbstractFunc {
    pub(crate) name_id: NameId,
    pub(super) name_span: Span,
    pub(crate) params: Vec<Expr>,
}

impl AbstractFunc {
    pub(crate) fn new(name_id: NameId, name_span: Span, params: Vec<Expr>) -> AbstractFunc {
        AbstractFunc {
            name_id,
            name_span,
            params,
        }
    }
}

#[derive(Debug)]
pub(crate) struct AbstractFuncDecl {
    pub(crate) name_id: NameId,
    pub(super) name_span: Span,
    pub(crate) params: Vec<Param>,
}

impl AbstractFuncDecl {
    pub(crate) fn new(name_id: NameId, name_span: Span, params: Vec<Param>) -> AbstractFuncDecl {
        AbstractFuncDecl {
            name_id,
            name_span,
            params,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Param {
    pub(crate) name_id: NameId,
    pub(crate) name_span: Span,
    pub(crate) type_expr: TypeExpr,
}

impl Param {
    pub(crate) fn new(name_id: NameId, name_span: Span, type_expr: TypeExpr) -> Param {
        Param {
            name_id,
            name_span,
            type_expr,
        }
    }
}

//TODO:
#[derive(Debug)]
pub(crate) struct AbstractFieldDecl {
    pub(crate) name_id: NameId,
    pub(crate) fields: Vec<Expr>,
}

// impl AbstractFieldDecl {
//     pub(crate) fn new(base: Box<Expr>, fields: Vec<Expr>) -> AbstractFieldDecl {
//         AbstractFieldDecl { base, fields }
//     }
// }

#[derive(Debug)]
pub(crate) struct AbstractAlias {
    // Only using this because of the span
    pub(crate) name_id: NameId,
    pub(crate) name_span: Span,
    pub(crate) params: Vec<TypeExpr>,
    pub(crate) conds: Vec<Expr>,
    pub(crate) args: Vec<SpannedInnerArgs>,
}

impl AbstractAlias {
    pub(crate) fn new(
        name_id: NameId,
        name_span: Span,
        params: Vec<TypeExpr>,
        conds: Vec<Expr>,
        args: Vec<SpannedInnerArgs>,
    ) -> AbstractAlias {
        AbstractAlias {
            name_id,
            name_span,
            params,
            conds,
            args,
        }
    }
}

#[derive(Debug)]
pub(crate) struct AbstractFieldAccess {
    pub(crate) base: Box<Expr>,
    pub(crate) field: NameId,
}

impl AbstractFieldAccess {
    pub(crate) fn new(base: Box<Expr>, field: NameId) -> AbstractFieldAccess {
        AbstractFieldAccess { base, field }
    }
}

#[derive(Debug)]
pub(crate) struct Unary {
    pub(crate) op: UnaryOp,
    pub(crate) expr: Box<Expr>,
}

impl Unary {
    pub(crate) fn new(op: UnaryOp, expr: Box<Expr>) -> Unary {
        Unary { op, expr }
    }
}

#[derive(Debug)]
pub(crate) enum UnaryOp {
    Not,
    Negate,
}

#[derive(Debug)]
pub(crate) struct AbstractGeneric {
    pub(crate) base: NameId,
    // Change to tuple or something alike since max 2?
    pub(crate) args: Vec<TypeExpr>,
}

impl AbstractGeneric {
    pub(crate) fn new(base: NameId, args: Vec<TypeExpr>) -> AbstractGeneric {
        AbstractGeneric { base, args }
    }
}
