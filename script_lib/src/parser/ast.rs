use common::symbols::{InnerArgs, NameId};

// Going for convention...
// Aliases too.

#[derive(Debug)]
pub enum Item {
    Bind(AbstractBind),
    // DO I SECTION THIS?
    Var(AbstractType),
    Struct(AbstractStruct),
    Enum(AbstractEnum),
    // HOW DO I MAKE CONSTANTS
    // You don't
    // TODO: Do we need this?
    // Func(AbstractFunc),
}

#[derive(Debug)]
pub enum Expr {
    Var(NameId),
    Number(usize),
    Literal(NameId),
    Call(Call),
    Unary(Unary),
}

#[derive(Debug)]
pub struct Call {
    pub(crate) callee: Box<Expr>,
    // Vec?
    pub(crate) exprs: Vec<Expr>,
}

impl Call {
    pub fn new(callee: Box<Expr>, exprs: Vec<Expr>) -> Call {
        Call { callee, exprs }
    }
}

// WHAT IS A TUPLE I HAVE NOT HEARD OF THAT BEFORE I AM NEW TO THINKING HAS ANYONE THOUGHT BEFORE?
#[derive(Debug)]
pub enum TypeExpr {
    Var(NameId),
    //_Generic
    Generic(Generic),
    Any,
}

// Maybe put in enum exclusively if not needed outside

#[derive(Debug)]
pub struct AbstractGeneric {
    pub(crate) name_id: NameId,
    pub(crate) args: Box<TypeExpr>,
}

impl AbstractGeneric {
    pub fn new(name_id: NameId, args: Box<TypeExpr>) -> AbstractGeneric {
        AbstractGeneric { name_id, args }
    }
}

// public abstract class AbstractBind {}
#[derive(Debug)]
pub struct AbstractBind {
    pub(crate) name_id: NameId,
}

impl AbstractBind {
    pub fn new(name_id: NameId) -> AbstractBind {
        AbstractBind { name_id }
    }
}

#[derive(Debug)]
pub struct AbstractType {
    pub(crate) name_id: NameId,
    pub(crate) ty: TypeExpr,
    pub(crate) args: Vec<InnerArgs>,
    pub(crate) conds: Vec<Expr>,
}

impl AbstractType {
    pub fn new(
        name_id: NameId,
        ty: TypeExpr,
        args: Vec<InnerArgs>,
        conds: Vec<Expr>,
    ) -> AbstractType {
        AbstractType {
            name_id,
            ty,
            args,
            conds,
        }
    }
}

#[derive(Debug)]
pub struct AbstractStruct {
    pub(crate) name_id: NameId,
    pub(crate) args: Vec<InnerArgs>,
    pub(crate) conds: Vec<Expr>,
    pub(crate) fields: Vec<AbstractType>,
}

impl AbstractStruct {
    pub fn new(
        name_id: NameId,
        args: Vec<InnerArgs>,
        conds: Vec<Expr>,
        //TODO: Change both enum and struct of field
        fields: Vec<AbstractType>,
    ) -> AbstractStruct {
        AbstractStruct {
            name_id,
            args,
            conds,
            fields,
        }
    }
}

#[derive(Debug)]
pub struct AbstractEnum {
    // Would be SymbolId in symbol table anyways
    pub(crate) name_id: NameId,
    pub(crate) args: Vec<InnerArgs>,
    pub(crate) conds: Vec<Expr>,
    pub(crate) variants: Vec<Variant>,
}

impl AbstractEnum {
    pub fn new(
        name_id: NameId,
        args: Vec<InnerArgs>,
        // I'm scared
        conds: Vec<Expr>,
        variants: Vec<Variant>,
    ) -> AbstractEnum {
        AbstractEnum {
            name_id,
            args,
            conds,
            variants,
        }
    }
}

// Hold that thought
#[derive(Debug)]
pub struct Variant {
    pub(crate) name_id: NameId,
    // I think this is right?
    pub(crate) ty: Option<TypeExpr>,
    pub(crate) args: Vec<InnerArgs>,
    pub(crate) conds: Vec<Expr>,
}

impl Variant {
    pub fn new(
        name_id: NameId,
        // I think this is right?
        ty: Option<TypeExpr>,
        args: Vec<InnerArgs>,
        conds: Vec<Expr>,
    ) -> Variant {
        Variant {
            name_id,
            ty,
            args,
            conds,
        }
    }
}

impl From<AbstractType> for Variant {
    fn from(ty: AbstractType) -> Self {
        todo!();
    }
}

#[derive(Debug)]
pub struct AbstractFunc {
    pub(crate) name_id: NameId,
    pub(crate) params: Vec<Expr>,
}

impl AbstractFunc {
    pub fn new(name_id: NameId, params: Vec<Expr>) -> AbstractFunc {
        AbstractFunc { name_id, params }
    }
}

#[derive(Debug)]
pub struct Unary {
    pub(crate) op: UnaryOp,
    pub(crate) expr: Box<Expr>,
}

impl Unary {
    pub fn new(op: UnaryOp, expr: Box<Expr>) -> Unary {
        Unary { op, expr }
    }
}

#[derive(Debug)]
pub enum UnaryOp {
    Not,
}

#[derive(Debug)]
pub struct Generic {
    pub(crate) base: NameId,
    // Change to tuple or something alike?
    pub(crate) args: Vec<TypeExpr>,
}

impl Generic {
    pub fn new(base: NameId, args: Vec<TypeExpr>) -> Generic {
        Generic { base, args }
    }
}
