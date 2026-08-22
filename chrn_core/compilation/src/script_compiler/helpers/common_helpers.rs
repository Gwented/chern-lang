use chrn_utils::id_types::{AstId, InternedId, ScopeId, SymbolId};

use crate::{
    lookup::scopes::scopes_concepts::{AssociatedScopeKind, ScopeType},
    semantic::hir::hir_symbols::{Symbol, SymbolKind, SymbolOrigin},
};

#[derive(Debug)]
pub struct CommonSymbolBase {
    pub name_id: InternedId,
    pub sym_origin: SymbolOrigin,
    pub scope_origin: ScopeType,
    pub is_priv: bool,
    pub kind: CommonSymbolKind,
}

impl CommonSymbolBase {
    pub const fn new(
        name_id: InternedId,
        sym_origin: SymbolOrigin,
        scope_origin: ScopeType,
        is_priv: bool,
        kind: CommonSymbolKind,
    ) -> Self {
        Self {
            name_id,
            sym_origin,
            scope_origin,
            is_priv,
            kind,
        }
    }

    /// Helper to convert to symbol using the already present metadata
    pub fn to_sym(
        &self,
        sym_id: SymbolId,
        ast_id: Option<AstId>,
        associated_scope: Option<AssociatedScopeKind>,
        kind: SymbolKind,
    ) -> Symbol {
        Symbol::new(
            self.name_id,
            sym_id,
            ast_id,
            self.sym_origin,
            self.is_priv,
            associated_scope,
            self.scope_origin,
            kind,
        )
    }
}

#[derive(Debug)]
pub enum CommonSymbolKind {
    Namespace(&'static [CommonSymbolBase]),
    ExternType,
}

// ?
pub enum CommonKind {
    Namespace(CommonNamespaceParts),
    Symbol(CommonSymbolParts),
}

// sym_id
// ast_id
/// General abstraction layer to create symbols based off of all not dynamic information in `Symbol`
pub struct CommonSymbolParts {
    pub name_id: InternedId,
    pub kind: SymbolKind,
    pub sym_origin: SymbolOrigin,
    pub scope_origin: ScopeType,
    pub is_priv: bool,
}

impl CommonSymbolParts {
    pub const fn new(
        name_id: InternedId,
        kind: SymbolKind,
        sym_origin: SymbolOrigin,
        scope_origin: ScopeType,
        is_priv: bool,
    ) -> Self {
        Self {
            name_id,
            kind,
            sym_origin,
            scope_origin,
            is_priv,
        }
    }

    pub const fn to_sym(&self, sym_id: SymbolId, ast_id: Option<AstId>) -> Symbol {
        Symbol::new(
            self.name_id,
            sym_id,
            ast_id,
            self.sym_origin,
            self.is_priv,
            None,
            self.scope_origin,
            self.kind,
        )
    }
}

/// General abstraction layer to create datasets for instantiating a symbol namespace
pub struct CommonNamespaceParts {
    pub name_id: InternedId,
    pub sym_origin: SymbolOrigin,
    pub scope_origin: ScopeType,
    pub is_priv: bool,
    pub syms: &'static [CommonKind],
}

impl CommonNamespaceParts {
    pub const fn new(
        name_id: InternedId,
        sym_origin: SymbolOrigin,
        scope_origin: ScopeType,
        is_priv: bool,
        syms: &'static [CommonKind],
    ) -> Self {
        Self {
            name_id,
            sym_origin,
            scope_origin,
            is_priv,
            syms,
        }
    }

    pub const fn to_sym(
        &self,
        sym_id: SymbolId,
        ast_id: Option<AstId>,
        associated_scope_id: ScopeId,
    ) -> Symbol {
        Symbol::new(
            self.name_id,
            sym_id,
            ast_id,
            self.sym_origin,
            self.is_priv,
            Some(AssociatedScopeKind::Scope(associated_scope_id)),
            self.scope_origin,
            SymbolKind::Namespace,
        )
    }
}
