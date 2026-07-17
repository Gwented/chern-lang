// REPORTER IS BACK 🦅🦅🦅🦅🦅𐔌
use chrn_utils::{
    budget::mem_budget::{BudgetResult, MemoryBudget},
    source_map::source_diagnostic::SourceDiagnostic,
};

use crate::script_compiler::script_compiler_summary::ScriptCompilerSummary;

// Summary owns reporter or reporter owns summary? eagle emoji.
/// This exists in case other methods or fields are considered, but is just a Vec<Diagnostic>
/// wrapper as of right now
#[derive(Debug, Default)]
pub struct Reporter {
    /// Stored diagnostics
    pub diags: Vec<SourceDiagnostic>,
    // The suppressed diagnostic count is the "exceeded_amt" in budget
    pub diag_budget: MemoryBudget,
    // WHAT?
    /// Summary of what the compiler did today
    pub(crate) summary: ScriptCompilerSummary,
}

impl Reporter {
    // Should this track module count?
    pub const fn new(max_diags: usize) -> Reporter {
        Reporter {
            diags: Vec::new(),
            summary: ScriptCompilerSummary::new(),
            diag_budget: MemoryBudget::new(max_diags),
        }
    }

    // This is only done when the internals are ACTUALLY something that ONLY happens at crate level,
    // this isn't from Java hypnosis (I think?)
    pub const fn summary(&self) -> &ScriptCompilerSummary {
        &self.summary
    }

    // I think this count is WRONG because there is no consumption from budget that assumes the
    // caller is going to consume the rest.
    /// How many diagnostics were attempted to be pushed but failed due to it exceeding the budget
    pub const fn suppressed_diagnostics(&self) -> usize {
        self.diag_budget.amt_exceeded()
    }

    pub fn push_safe(&mut self, diag: SourceDiagnostic) -> bool {
        // Need to do something with consume() before using it so this stays checked.
        match self.diag_budget.consume(1) {
            BudgetResult::Stable | BudgetResult::LimitReached => {
                self.diags.push(diag);
                true
            }
            // Reward hacking my own semantics </3
            BudgetResult::Overage(_) | BudgetResult::Overflow => false,
        }
    }

    /// Checks if max budget has been exceeded before appending.
    ///
    /// `true` means there were no issues
    /// `false` means as many diagnostics as possible were appended, but there was an overage in budget
    ///
    /// This method by default assumes that the usage should be per-diagnostic, with no deeper
    /// control over if it should account for bytes. May change.
    // Maybe return ok and the amount of space left?
    pub fn append_safe(&mut self, diags: &mut Vec<SourceDiagnostic>) -> bool {
        // Budgeting is always done through the total amount of diagnostics rather than bytes. May
        // change to be more customizable. Is that necessary?
        let amt = diags.len();

        match self.diag_budget.checked_consume(amt) {
            BudgetResult::Stable | BudgetResult::LimitReached => {
                self.diags.append(diags);
                true
            }
            BudgetResult::Overage(overage) => {
                // let can_append = amt - overage;
                // dbg!(can_append, self.diag_budget.remaining());
                // panic!("Test me");
                for i in diags.drain(..self.diag_budget.remaining()) {
                    self.diags.push(i);
                }
                // Since the budget doesn't set itself to the limit the user must manually use the
                // set to limit after compensating for said limit.
                self.diag_budget.set_to_limit();

                false
            }
            BudgetResult::Overflow => false,
            // Ok(_) => {
            //     self.diags.append(diags);
            //     true
            // }
            // Err(overage_opt) => {
            //     self.budget.amt_exceeded = self.budget.amt_exceeded.saturating_add(amt);
            //     // If the overeage
            //     if let Some(overage) = overage_opt {
            //         let can_append = overage - amt;
            //         dbg!(can_append);
            //         panic!()
            //     } else {
            //         false
            //     }
            // }
        }
    }
}
