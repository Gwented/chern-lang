use common::symbols::{Cond, InnerArgs, NameId};

// Going for convention...
// Aliases too.

#[derive(Debug)]
pub enum Item {
    Bind(AbstractBind),
    Var(AbstractType),
    Struct(AbstractStruct),
    Enum(AbstractEnum),
    Func(AbstractFunc),
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
    callee: Box<Expr>,
    // Vec?
    expr: Vec<Expr>,
}

impl Call {
    pub fn new(callee: Box<Expr>, expr: Vec<Expr>) -> Call {
        Call { callee, expr }
    }
}

// Will likely remove many of these other intermediary variables.
// Just following guidance to ensure I don't trail off into the same insane workarounds as before.

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
    name_id: NameId,
    args: Box<TypeExpr>,
}

impl AbstractGeneric {
    pub fn new(name_id: NameId, args: Box<TypeExpr>) -> AbstractGeneric {
        AbstractGeneric { name_id, args }
    }
}

// public abstract class AbstractBind {}
#[derive(Debug)]
pub struct AbstractBind {
    name_id: NameId,
}

impl AbstractBind {
    pub fn new(name_id: NameId) -> AbstractBind {
        AbstractBind { name_id }
    }
}

//TODO:
//Abstract repre:
//name_id: NameId
//type_id: NameId
//args: Vec<InnerArgs>
//Vec<Cond>
#[derive(Debug)]
pub struct AbstractType {
    name_id: NameId,
    ty: TypeExpr,
    args: Vec<InnerArgs>,
    conds: Vec<Expr>,
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
    // Would be SymbolId in symbol table anyways
    name_id: NameId,
    args: Vec<InnerArgs>,
    conds: Vec<Expr>,
    fields: Vec<Field>,
}

impl AbstractStruct {
    pub fn new(
        name_id: NameId,
        args: Vec<InnerArgs>,
        conds: Vec<Expr>,
        //TODO: Change both enum and struct of field
        fields: Vec<Field>,
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
    name_id: NameId,
    args: Vec<InnerArgs>,
    conds: Vec<Expr>,
    variants: Vec<Field>,
}

impl AbstractEnum {
    pub fn new(
        name_id: NameId,
        args: Vec<InnerArgs>,
        // I'm scared
        conds: Vec<Expr>,
        variants: Vec<Field>,
    ) -> AbstractEnum {
        AbstractEnum {
            name_id,
            args,
            conds,
            variants,
        }
    }
}

// Shorter naming?
//name_id: NameId
//args: Vec<FuncArgs>
#[derive(Debug)]
pub struct AbstractFunc {
    name_id: NameId,
    params: Vec<Expr>,
}

impl AbstractFunc {
    pub fn new(name_id: NameId, params: Vec<Expr>) -> AbstractFunc {
        AbstractFunc { name_id, params }
    }
}

// HELP
#[derive(Debug)]
pub struct Unary {
    op: UnaryOp,
    expr: Box<Expr>,
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
    base: NameId,
    args: Vec<TypeExpr>,
}

impl Generic {
    pub fn new(base: NameId, args: Vec<TypeExpr>) -> Generic {
        Generic { base, args }
    }
}

// Same thing but go with it for type safety or something i don't know anymore
#[derive(Debug)]
pub struct Field {
    name_id: NameId,
    ty: TypeExpr,
}

impl Field {
    pub fn new(name_id: NameId, ty: TypeExpr) -> Field {
        Field { name_id, ty }
    }
}
