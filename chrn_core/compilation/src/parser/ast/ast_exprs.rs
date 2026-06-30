use chrn_utils::{
    id_types::{InternedId, SpannedContainer},
    source_map::source_span::SourceSpan,
};

use crate::{
    lexer::token::Notation,
    parser::ast::ast_concepts::{AbstractMemberAccess, BinaryOp, Unary},
};

#[derive(Debug)]
pub struct SpannedExpr {
    pub expr: Expr,
    pub span: SourceSpan,
}

impl SpannedExpr {
    pub fn new(expr: Expr, span: SourceSpan) -> SpannedExpr {
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
    Integer(InternedId, Notation),
    Float(InternedId, Notation),
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
    Array(ArrayExpr),
}

#[derive(Debug)]
pub(crate) struct CallExpr {
    pub(crate) name_id: InternedId,
    pub(crate) spanned_expr: Vec<SpannedExpr>,
}

impl CallExpr {
    pub(crate) fn new(name_id: InternedId, spanned_expr: Vec<SpannedExpr>) -> CallExpr {
        CallExpr {
            name_id,
            spanned_expr,
        }
    }
}

#[derive(Debug)]
pub struct ArrayExpr {
    pub elements: Vec<SpannedExpr>,
}

impl ArrayExpr {
    pub fn new(elements: Vec<SpannedExpr>) -> ArrayExpr {
        ArrayExpr { elements }
    }
}

// #[derive(Debug, Clone)]
// pub struct SpannedTypeExpr {
//     pub ty_expr: TypeExpr,
//     pub span: SourceSpan,
// }
//
// impl SpannedTypeExpr {
//     pub fn new(ty_expr: TypeExpr, span: SourceSpan) -> SpannedTypeExpr {
//         SpannedTypeExpr { ty_expr, span }
//     }
// }

#[derive(Debug, Clone)]
pub enum TypeExpr {
    Var(InternedId),
    Path(Vec<SpannedPathSegment>),
    Generic(Generic),
}

#[derive(Debug, Clone)]
pub struct SpannedPathSegment {
    pub kind: PathSegment,
    pub span: SourceSpan,
}

impl SpannedPathSegment {
    pub fn new(kind: PathSegment, span: SourceSpan) -> Self {
        SpannedPathSegment { kind, span }
    }
}

#[derive(Debug, Clone)]
pub enum PathSegment {
    Ident(InternedId),
    Generic(Generic),
}

// impl PathSegment {
//     pub fn as_type_expr_ref(&self) -> TypeExpr {
//         match self {
//             PathSegment::Ident(interned_id) => &TypeExpr::Var(*interned_id),
//             PathSegment::Generic(generic) => TypeExpr::Generic(&generic),
//         }
//     }
// }

#[derive(Debug, Clone)]
pub struct Generic {
    pub base: InternedId,
    // Change to tuple or something alike since max 2?
    pub inputs: Vec<SpannedContainer<TypeExpr>>,
}

impl Generic {
    pub fn new(base: InternedId, inputs: Vec<SpannedContainer<TypeExpr>>) -> Generic {
        Generic { base, inputs }
    }
}
