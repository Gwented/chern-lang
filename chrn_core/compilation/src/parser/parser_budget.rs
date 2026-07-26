use std::cell::Cell;

use chrn_utils::budget::{
    mem_budget::{BudgetResult, MemoryBudget},
    recursion_tracker::{RecursionTracker, RecursiveGuard},
};

/// Budget tracker specifically for parser that tracks expression nodes and uses `RecursiveTracker`
#[derive(Debug)]
pub(super) struct ParserBudget {
    // Is cell so that this structure can be borrowed and internally mutated without borrow checker
    // issues since this is just a counter.
    pub(super) recursion_tracker: RecursionTracker,
    node_budget: MemoryBudget,
}

impl ParserBudget {
    pub(super) fn new(recursion_limit: u16, node_limit: usize) -> ParserBudget {
        ParserBudget {
            recursion_tracker: RecursionTracker::new(recursion_limit),
            node_budget: MemoryBudget::new(node_limit),
        }
    }

    // Um
    /// Creates recursive-depth tracking guard
    ///
    /// Returns `Ok` guard if the limit has not been reached.
    /// Returns `Err` if an extra guard would exceed `self.limit`
    pub(super) fn increase_depth(&self) -> Result<RecursiveGuard<'_>, ()> {
        self.recursion_tracker.increase_depth()
    }

    pub(super) fn increase_node(&mut self) -> Result<(), ()> {
        todo!("NOT DONE YET");
        match self.node_budget.consume(1) {
            BudgetResult::Stable => Ok(()),
            BudgetResult::Overage(_) | BudgetResult::LimitReached => Err(()),
            // Maybe don't return budget result from consume() calls?
            BudgetResult::Overflow => unreachable!(),
        }
    }
}
