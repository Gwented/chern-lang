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
