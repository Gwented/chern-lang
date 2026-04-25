use std::fmt::Display;

use chrn_core::id_types::ScopeId;

use crate::semantic::representation::Table;

// Neutral, var, nest, and complex scopes can only access variables from neutral and nest.
// Override is unsure
#[derive(Debug)]
pub(crate) struct Scope {
    pub(crate) table: Table,
    pub(crate) scope_id: ScopeId,
    pub(crate) scope_type: ScopeType,
}

impl Scope {
    pub(crate) fn new(scope_id: ScopeId, scope_type: ScopeType) -> Scope {
        Scope {
            table: Table::new(),
            scope_id,
            scope_type,
        }
    }
}

#[derive(Debug)]
pub(crate) struct ScopeInfo {
    scope_id: ScopeId,
    scope_type: ScopeType,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum ScopeType {
    Neutral,
    Var,
    Nest,
    Complex,
    Override,
}

impl ScopeType {
    pub(crate) fn accessible_scopes(&self) -> Vec<ScopeType> {
        match self {
            ScopeType::Neutral => vec![ScopeType::Neutral],
            ScopeType::Var | ScopeType::Nest | ScopeType::Complex => {
                vec![ScopeType::Neutral, ScopeType::Nest]
            }
            ScopeType::Override => vec![ScopeType::Nest],
        }
    }
}

// Not using Formattable unless needed globally
impl Display for ScopeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScopeType::Neutral => write!(f, "neutral"),
            ScopeType::Var => write!(f, "var"),
            ScopeType::Nest => write!(f, "nest"),
            ScopeType::Complex => write!(f, "complex"),
            ScopeType::Override => write!(f, "override"),
        }
    }
}
