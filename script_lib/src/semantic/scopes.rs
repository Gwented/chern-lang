use std::fmt::Display;

use chern_core::id_types::{AstId, NameId, ScopeId};

use crate::semantic::representation::Table;

// ScrumMaster
// Purely exists for the ability to use methods to wrap scope indexing
// Nesting nesting tiesetntnesting
#[derive(Debug)]
pub(crate) struct ScopeManager {
    pub(crate) scopes: Vec<Scope>,
}

impl ScopeManager {
    pub(crate) fn new() -> ScopeManager {
        ScopeManager { scopes: Vec::new() }
    }

    /// Get's the `ScopeId` with no assumption of it existing.
    ///
    /// This method exists along with extract_scope_id due to cross module namespace checking not
    /// innately confirming whether or not it contains a particular `ScopeType`
    pub(crate) fn get_scope_id(&self, scope_type: ScopeType) -> Option<ScopeId> {
        self.find_scope(scope_type).map(|s| s.scope_id)
    }

    /// Get's the `ScopeId` assuming that the scope already exists. Panics otherwise.
    ///
    /// This exists because if the current module has something like a typdef in the semantic stage,
    /// that means the parser itself already checked if it was legal grammar-wise.
    pub(crate) fn extract_scope_id(&self, scope_type: ScopeType) -> ScopeId {
        self.find_scope(scope_type)
            .expect("Either semantic broke, parser broke, or modules broke")
            .scope_id
    }

    /// Get's scope using a `ScopeId`
    pub(crate) fn get_scope(&self, scope_id: ScopeId) -> &Scope {
        &self.scopes[scope_id.id]
    }

    /// Get's mutably scope using a `ScopeId`
    pub(crate) fn get_scope_mut(&mut self, scope_id: ScopeId) -> &mut Scope {
        &mut self.scopes[scope_id.id]
    }

    /// Pushes new scope with given scope type and returns the `ScopeId`. If the scope already
    /// exists then it returns the existent `ScopeId`.
    pub(crate) fn push_scope(&mut self, scope_type: ScopeType) -> ScopeId {
        if let Some(scope) = self.find_scope(scope_type) {
            return scope.scope_id;
        }

        let scope_id = ScopeId::new(self.scopes.len());
        self.scopes.push(Scope::new(scope_id, scope_type));

        scope_id
    }

    // And type?
    /// Checks if the name id corresponds to an ast id within the given `ScopeType`.
    /// Returns a tuple of the `AstId` and `ScopeType` the `NameId` was found in. Returns None if
    /// no scopes contain the given `NameId`.
    pub(crate) fn get_ast_id(
        &self,
        name_id: NameId,
        scope_type: ScopeType,
    ) -> Option<(AstId, ScopeType)> {
        // I don't think this can fail. Should maybe expect for clarity.
        let allowed_scope_types = scope_type.accessible_scopes();

        // Loops over all allowed scopes and checks their individual namespaces
        for allowed_scope_type in allowed_scope_types {
            // In this scenario the scope may or may not exist since this could be used from
            // another module
            if let Some(scope) = self.find_scope(allowed_scope_type) {
                for (current_ast_id, current_name_id) in &scope.table.name_ids {
                    if *current_name_id == name_id {
                        return Some((*current_ast_id, allowed_scope_type));
                    }
                }
            }
        }

        None
    }

    /// Returns Some scope if it exists, None otherwise
    fn find_scope(&self, scope_type: ScopeType) -> Option<&Scope> {
        for scope in &self.scopes {
            if scope.scope_type == scope_type {
                return Some(scope);
            }
        }

        None
    }
}
// So to search we would need the module id and namespace, meaning we now need to check which scope
// a given module starts at.

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
    fn accessible_scopes(&self) -> Vec<ScopeType> {
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
