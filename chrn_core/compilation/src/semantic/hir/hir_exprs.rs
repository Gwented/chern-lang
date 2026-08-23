use chrn_utils::{
    id_types::{AstId, ExprId, SymbolId, TypeId, ValueId},
    source_map::source_span::SourceSpan,
};

use crate::parser::ast::ast_concepts::{BinaryOp, UnaryOp};

// Maybe this should be a member?
#[derive(Debug)]
pub struct Param {
    pub sym_id: SymbolId,
    //FIX: More like "FieldId"
    //
    pub type_id: TypeId,
    // Should become SpanId maybe.
    pub ast_id: AstId,
}

impl Param {
    pub fn new(sym_id: SymbolId, type_id: TypeId, ast_id: AstId) -> Param {
        Param {
            sym_id,
            type_id,
            ast_id,
        }
    }
}

#[derive(Debug)]
pub struct ResolvedExpr {
    /// Type of expr
    pub type_id: TypeId,
    /// `ExprHir` of `self`
    pub expr_hir: ExprHir,
    /// What other exprs this expr is dependent on
    pub inputs: Vec<ExprId>,
    /// Whether or not there is a symbol higher upon the tree of exprs.
    ///
    /// User -> User -> None
    pub user: Option<ExprId>,
    pub meta: ResolvedExprMetadata,
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
        meta: ResolvedExprMetadata,
        inputs: Vec<ExprId>,
    ) -> ResolvedExpr {
        ResolvedExpr {
            type_id,
            expr_hir,
            inputs,
            meta,
            user: None,
            val_id,
        }
    }
}

#[derive(Debug)]
pub enum ResolvedExprMetadata {
    User(SourceSpan),
    Generated,
}

impl ResolvedExprMetadata {
    pub fn expect_user(&self) -> SourceSpan {
        match self {
            ResolvedExprMetadata::User(span) => *span,
            ResolvedExprMetadata::Generated => {
                panic!(
                    "Expected `{:?}`, found `{self:?}`",
                    ResolvedExprMetadata::Generated
                )
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExprHir {
    Val(ValueId),
    Var(SymbolId),
    /// alias default(x) = [Equals(x = 3)]
    /// x = `SymbolId`, 5 = `ExprId`
    Default(ExprId, ExprId),
    // MemberAccess(),
    // Um
    /// Caller, arguments
    Call(ExprId, Vec<ExprId>),
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
    Array(Vec<ExprId>),
}

//TEST:
pub(crate) enum PossibleMember {
    Type(TypeId),
    // Member(MemberId),
    Var(ValueId),
    Nothing,
}
