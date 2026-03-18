//NOTE: MAY MAKE EXPRESSION THAT HELPS RESOLVE SEMANTIC TYPES MORE CLEANLY
use common::symbols::{InnerArgs, NameId, Span, SpannedInnerArgs};

// Going for convention...
// Aliases too.

#[derive(Debug)]
pub struct Program {
    // MAYBE SHOULDN'T BE A NAME ID I DONT KNOW
    pub bind: Option<NameId>,
    pub items: Vec<Item>,
}

impl Program {
    pub(crate) fn new() -> Program {
        Program {
            bind: None,
            items: Vec::new(),
        }
    }

    //TEST: Oh my java.
    pub(crate) fn set_bind(&mut self, bind: NameId) {
        self.bind = Some(bind);
    }

    pub(crate) fn has_bind(&self) -> bool {
        self.bind.is_some()
    }
}

#[derive(Debug)]
pub enum Item {
    //                                                 name: str [!IsEmpty, Range(0,5)]
    //TODO: Should these have spans? Do we REALLY want ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    //
    Var(AbstractTypeDef),
    Struct(AbstractStruct),
    Enum(AbstractEnum),
    // Func(AbstractFunc),
}

#[derive(Debug)]
pub(crate) enum Expr {
    Var(NameId, Span),
    // Staying capped at i64 and f64 for pacing purposes
    // TODO: Need to likely carry notation here
    Integer(i64, Span),
    Float(f64, Span),
    Str(NameId, Span),
    Call(Call, Span),
    FieldAccess(AbstractFieldAccess, Span),
    Unary(Unary, Span),
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

// WHAT IS A TUPLE I HAVE NOT HEARD OF THAT BEFORE I AM NEW TO THINKING HAS ANYONE THOUGHT BEFORE?
#[derive(Debug)]
pub(crate) enum TypeExpr {
    Var(NameId, Span),
    //_Generic
    Generic(AbstractGeneric, Span),
    Any(Span),
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
// ONE HUNDRED TWENTY BYTES WHAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAT
// WHAT
//  This is fine, none of this is bad. None of it. NONE.
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
        conds: Vec<Expr>,
        glob_args: Vec<SpannedInnerArgs>,
    ) -> AbstractEnum {
        AbstractEnum {
            name_id,
            name_span,
            variants,
            glob_conds: conds,
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
pub struct AbstractFunc {
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

// Please no SpannedNameId
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
    // Negate
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
