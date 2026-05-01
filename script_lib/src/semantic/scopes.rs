use std::fmt::Display;

use chrn_utils::id_types::ScopeId;

use crate::semantic::representation::Table;

// Neutral, var, nest, and complex scopes can only access variables from neutral and nest.
// Override is unsure
#[derive(Debug)]
pub struct Scope {
    pub table: Table,
    pub scope_id: ScopeId,
    pub scope_type: ScopeType,
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

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ScopeType {
    Neutral,
    Var,
    Nest,
    Complex,
    Override,
}

impl ScopeType {
    /// Direct representation of how the language views scope accessibility.
    /// `needs_global` purely exists for all scope accessibility reasons
    pub(crate) fn accessible_scopes(&self, needs_global: bool) -> Vec<ScopeType> {
        if needs_global {
            return vec![
                ScopeType::Neutral,
                ScopeType::Var,
                ScopeType::Nest,
                ScopeType::Complex,
                ScopeType::Override,
            ];
        }

        match self {
            // Mainly for internal usage, not an actual program recognizable scope
            // Neutral can only access neutral because this section is purely for declaring and
            // using in other sections
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
