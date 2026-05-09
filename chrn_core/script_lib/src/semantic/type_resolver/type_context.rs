use std::collections::{HashMap, VecDeque};

use chrn_utils::id_types::{ExprId, ModuleId, SymbolId};

// The idea here is to store a symbol that points to an expression at the bottom of a tree.
// So, let y = x + 2 would store x in the symbol queue, and he expression id "x" within this
// expression, basically marking where to start from as to avoid starting at the symbol level.
//
// When let x = 2 happens, x is searched for in the queue, we see the expression id for the x in
// y = x + 2, so now it traverses down, sees expression add, resolves the add, then stops whenever
// there are no more users.
// HELP
#[derive(Debug)]
pub struct TypeContext {
    // Caches checks each time a queued symbol is resolved
    pub(super) needs_check: bool,
    /// Queue of symbols that another symbol depends on which has not been resolved yet.
    /// Example: If we have, "let y = x + 2" we do not know the value of x yet, so x is stored as a
    /// symbol that is unresolved, y is pushed as a symbol that will be resolved later within the
    /// user_queue.
    pub(super) sym_queue: HashMap<SymbolId, PendingSymbol>,
    /// Queue to allow for actively keeping note of what references are
    /// still not referenced. This is a cached way of checking if there are any
    /// symbols left unresolved without checking users directly.
    // HashMap seems like a lot here but needs O(1) remove and insertion so a quick !is_empty can
    // be done without spending time re-ordering an array while resolving pending users

    // Possibly will be an ExprDetail structure that holds the span, and a semantic error that
    // would be given to it so that the expression can be reported
    pub(super) user_queue: VecDeque<PendingUser>,
    // Symbol id of a symbol that is unresolved, and what other symbols it depends on
}

impl TypeContext {
    pub fn new() -> TypeContext {
        TypeContext {
            needs_check: false,
            sym_queue: HashMap::new(),
            user_queue: VecDeque::new(),
        }
    }

    // This explanation is confusing!
    /// Takes an input of the symbol that is pending and the expression that is pending.
    /// This method is intended to prevent boiler-plate of checking if the symbol exists each time.
    pub(super) fn store_pending_expr(&mut self, sym_id: SymbolId, pending_expr: PendingExpr) {
        if let Some(pending_sym) = self.sym_queue.get_mut(&sym_id) {
            pending_sym.pending_exprs.push(pending_expr);
        } else {
            let pending_sym = PendingSymbol::new(vec![pending_expr]);
            self.sym_queue.insert(sym_id, pending_sym);
        }
    }
}
/// Struct to represent a symbol that is has users. Mainly exists so there is one point where
/// "is_resolved" can be cached as opposed to giving it to individual expressions
#[derive(Debug)]
pub(super) struct PendingSymbol {
    pub(super) is_resolved: bool,
    /// All symbols the user is waiting on.
    pub(super) pending_exprs: Vec<PendingExpr>,
}

impl PendingSymbol {
    pub(super) fn new(pending_exprs: Vec<PendingExpr>) -> PendingSymbol {
        PendingSymbol {
            is_resolved: false,
            pending_exprs,
        }
    }
}

#[derive(Debug)]
// Unsure if this is needed entirely since pending symbols would seemingly be the only thing
// needed? Error reporting is better if this stays here though since- Actually, cache.
/// Struct to represent a symbol that is awaiting the value of a symbol it is dependent on.
pub(super) struct PendingUser {
    /// The user's `SymbolId`
    pub(super) sym_id: SymbolId,
    // This seems interweaved and odd maybe this should just go to symbol directly instead of this
    // exprid misdirection.
    /// The all symbols the user is waiting on.
    pub(super) deps: Vec<SymbolId>,
}

impl PendingUser {
    pub(super) fn new(sym_id: SymbolId, deps: Vec<SymbolId>) -> PendingUser {
        PendingUser { sym_id, deps }
    }
}

#[derive(Debug)]
/// Struct to represent an expr that any amount of other expression are waiting for so that they can
/// be resolved.
pub(super) struct PendingExpr {
    pub(super) pending_id: ExprId,
    pub(super) parent_sym: SymbolId,
}

impl PendingExpr {
    pub(super) fn new(pending_id: ExprId, parent_sym: SymbolId) -> PendingExpr {
        PendingExpr {
            pending_id,
            parent_sym,
        }
    }
}
