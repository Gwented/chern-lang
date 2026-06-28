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
    // NOTE: Considering making a typesafe wrapper to unknown check explicitly
    pub type_id: TypeId,
    pub expr_hir: ExprHir,
    // May store these elsewhere depending on um...uh..unreachable!()
    pub inputs: Vec<ExprId>,
    // Should be one
    pub user: Option<ExprId>,
    pub span: SourceSpan,
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
        span: SourceSpan,
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
