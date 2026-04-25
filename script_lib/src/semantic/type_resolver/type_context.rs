use std::collections::{HashMap, VecDeque};

use chrn_core::id_types::{ExprId, SymbolId};

// May turn this from expression that depends on a symbol that has users, to a symbol that depends
// on a symbol for simplicity.
#[derive(Debug)]
pub struct TypeContext {
    /// Queue of symbols that another symbol depends on which has not been resolved yet.
    /// Example: If we have, "let y = x + 2" we do not know the value of x yet, so x is stored as a
    /// symbol that is unresolved, y is pushed as a symbol that will be resolved later within the
    /// user_queue.
    pub(super) expr_queue: HashMap<SymbolId, PendingExpr>,
    /// Queue to allow for actively keeping note of what references are
    /// still not referenced. This is a cached way of checking if there are any
    /// symbols left unresolved without checking users directly.
    // Maybe just use a hashy
    pub(super) user_queue: VecDeque<PendingUser>,
}

impl TypeContext {
    pub fn new() -> TypeContext {
        TypeContext {
            expr_queue: HashMap::new(),
            user_queue: VecDeque::new(),
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
/// Struct to represent a symbol that any amount of other symbols are waiting for so that they can
/// be resolved.
pub(super) struct PendingExpr {
    //
    pub(super) pending_expr_id: ExprId,
    pub(super) is_resolved: bool,
    pub(super) users: Vec<SymbolId>,
}

impl PendingExpr {
    pub(super) fn new(
        pending_expr_id: ExprId,
        is_resolved: bool,
        users: Vec<SymbolId>,
    ) -> PendingExpr {
        PendingExpr {
            pending_expr_id,
            is_resolved,
            users,
        }
    }
}

// #[derive(Debug)]
// pub(super) enum ExprResult {
//     Resolved(ExprId),
//     // May change to just outright give the caller.
//     /// SymbolId of the expression that was not reolved. Given, "let y = x + 2", x would be
//     /// unresolved so x is returned so the caller, y, can store itself as a user of x.
//     Unresolved(SymbolId),
// }
