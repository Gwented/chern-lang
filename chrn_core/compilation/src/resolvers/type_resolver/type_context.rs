use std::collections::HashMap;

use chrn_utils::id_types::{ExprId, SymbolId};

// The idea here is to store a symbol that points to an expression at the bottom of a tree.
// So, let y = x + 2 would store x in the symbol queue, and he expression id "x" within this
// expression, basically marking where to start from as to avoid starting at the symbol level.
//
// When let x = 2 happens, x is searched for in the queue, we see the expression id for the x in
// y = x + 2, so now it traverses down, sees expression add, resolves the add, then stops whenever
// there are no more users.
#[derive(Debug)]
pub struct TypeContext {
    /// Whether or not the context should be checked. This is to prevent unconditional checks where
    /// possible.
    pub(super) needs_check: bool,
    /// Queue of symbols that another symbol depends on which has not been resolved yet.
    /// Example: If we have, "let y = x + 2" we do not know the value of x yet, so x is stored as a
    /// symbol that is unresolved, y is pushed as a symbol that will be resolved later within the
    /// user_queue.
    pub(super) sym_queue: HashMap<SymbolId, PendingSymbol>,
}

impl TypeContext {
    pub fn new() -> TypeContext {
        TypeContext {
            needs_check: false,
            sym_queue: HashMap::new(),
        }
    }

    // This explanation is confusing!
    /// Takes the symbol that is pending and the expression that is pending.
    /// This method is intended to prevent boiler-plate of checking if the symbol exists inside
    /// `pending_exprs` each time
    pub(super) fn store_pending_expr(&mut self, sym_id: SymbolId, pending_expr: PendingExpr) {
        if let Some(pending_sym) = self.sym_queue.get_mut(&sym_id) {
            pending_sym.pending_exprs.push(pending_expr);
        } else {
            let pending_sym = PendingSymbol::new(vec![pending_expr]);
            self.sym_queue.insert(sym_id, pending_sym);
        }
    }
}
/// Struct to represent a symbol has users but isn't resolved yet. Mainly exists so that metadata
/// can be associated with a `Symbol` without making every expr own it's own resolved state, which
/// would just be noise and wasted byte padding outside of type resolution.
#[derive(Debug)]
pub(super) struct PendingSymbol {
    pub(super) has_const_val: bool,
    pub(super) has_resolved_ty: bool,
    /// All symbols the user is waiting on.
    pub(super) pending_exprs: Vec<PendingExpr>,
}

impl PendingSymbol {
    pub(super) fn new(pending_exprs: Vec<PendingExpr>) -> PendingSymbol {
        PendingSymbol {
            has_const_val: false,
            has_resolved_ty: false,
            pending_exprs,
        }
    }
}

/// Struct to represent an expr that any amount of other expression are waiting for so that they can
/// be resolved.
#[derive(Debug)]
pub(super) struct PendingExpr {
    pub(super) pending_id: ExprId,
    pub(super) kind: PendingExprKind,
}

/// Encodes all possible expr kinds that require different handling
#[derive(Debug)]
pub(super) enum PendingExprKind {
    Parent(ParentStateBase),
    Standing(StandingExprState),
}

/// Expr that is pending but has no parent ties to update.
///
/// For example, something like [x,y,z] cannot have a cycle
/// because there is no parent.
///
/// It's only responsibility is to wait for resolution and update accordingly.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(super) enum StandingExprState {
    Unresolved,
    Resolved {
        has_resolved_ty: bool,
        has_const_val: bool,
    },
    Error,
}

impl PendingExpr {
    pub(super) fn new(pending_id: ExprId, kind: PendingExprKind) -> PendingExpr {
        PendingExpr { pending_id, kind }
    }
}

/// Parent state intended to guide resolution and be changed by the child expression's
/// themselves.
#[derive(Debug)]
pub(super) struct ParentStateBase {
    pub(super) parent_sym_id: SymbolId,
    pub(super) state: ParentState,
}

impl ParentStateBase {
    pub(super) fn new(parent_sym_id: SymbolId, state: ParentState) -> Self {
        Self {
            parent_sym_id,
            state,
        }
    }
}

// -- HELPERS --
/// State of parent expression intended guide resolution and be changed by the child expression's
/// themselves
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub(super) enum ParentState {
    /// Parent has no resolved type and value.
    Unresolved,
    ///
    Resolved {
        has_resolved_ty: bool,
        has_const_val: bool,
    },
    ///
    Notified {
        has_resolved_ty: bool,
        has_const_val: bool,
    },
    Error,
}

/// Helper struct to use for transporting parent-related data rather than using an unnamed tuple.
#[derive(Debug)]
pub(super) struct ParentInfo {
    /// The parent's `SymbolId` that is stored as pending
    pub pending_sym_id: SymbolId,
    pub has_resolved_ty: bool,
    pub has_const_val: bool,
}

impl ParentInfo {
    pub(super) fn new(
        pending_sym_id: SymbolId,
        has_resolved_ty: bool,
        has_const_val: bool,
    ) -> ParentInfo {
        ParentInfo {
            pending_sym_id,
            has_resolved_ty,
            has_const_val,
        }
    }
}
