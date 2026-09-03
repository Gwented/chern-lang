use chrn_utils::{
    id_types::InternedId, source_map::source_span::SourceSpan, utils::containers::SpannedContainer,
};

use crate::parser::ast::ast_exprs::{PathSegment, SpannedExpr, TypeExpr};

/// Holds general statements.
/// Does not hold a statement like `let` due to it only being available in `neutral`, which would make
/// it less so a general statement and more so a concept entirely isolated to one section. Could
/// change if more broadly used.
#[derive(Debug)]
pub enum AstStmt {
    OptAssignment(AbstractOptionAssignment),
    MultiAssignType(AbstractTypeMultiAssign),
}

/// Option assignment ast representation
/// "outer { opt_name = [2, 3] }"
#[derive(Debug)]
pub struct AbstractOptionAssignment {
    /// Name of structural type to configure
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    /// Must be an `ArrayExpr`
    pub array_expr: SpannedExpr,
}

impl AbstractOptionAssignment {
    pub fn new(
        name_id: InternedId,
        name_span: SourceSpan,
        array_expr: SpannedExpr,
    ) -> AbstractOptionAssignment {
        AbstractOptionAssignment {
            name_id,
            name_span,
            array_expr,
        }
    }
}

/// Multi-assignment for type exprs.
///
/// "TypeExpr, TypeExpr, TypeExpr = TypeExpr"
/// "i32, u32 = java::int"
#[derive(Debug, Clone)]
pub struct AbstractTypeMultiAssign {
    // Need better terms
    /// -> (i32, u32, u8) = java::int
    pub to_assign: Vec<SpannedContainer<TypeExpr>>,
    /// i32, u32, u8 = (java::int) <-
    pub assigned_to: Vec<SpannedContainer<PathSegment>>,
}

impl AbstractTypeMultiAssign {
    pub fn new(
        to_assign: Vec<SpannedContainer<TypeExpr>>,
        assigned_to: Vec<SpannedContainer<PathSegment>>,
    ) -> Self {
        Self {
            to_assign,
            assigned_to,
        }
    }
}
