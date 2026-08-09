// REPORTER IS BACK 🦅🦅🦅🦅🦅𐔌
use chrn_utils::{
    budget::mem_budget::{BudgetResult, MemoryBudget},
    source_map::source_diagnostic::{SourceDiagnostic, SourceDiagnosticSummary},
};

use crate::script_compiler::script_compiler_summary::ScriptCompilerSummary;

// Summary owns reporter or reporter owns summary? eagle emoji.
/// Diagnostic and summary holder for compiler activity
#[derive(Debug, Default)]
pub struct Reporter {
    /// Stored diagnostics
    pub(crate) diag_summary: SourceDiagnosticSummary,
    // The suppressed diagnostic count is the "exceeded_amt" in budget
    pub(crate) diag_budget: MemoryBudget,
    // Today?
    // Yuppy
    /// Summary of what the compiler did today
    pub(crate) summary: ScriptCompilerSummary,
}

impl Reporter {
    // Should this track module count?
    pub fn new(max_diags: usize) -> Reporter {
        Reporter {
            diag_summary: SourceDiagnosticSummary::default(),
            summary: ScriptCompilerSummary::new(),
            diag_budget: MemoryBudget::new(max_diags),
        }
    }

    // This is only done when the internals are ACTUALLY something that ONLY happens at crate level,
    // this isn't from Java hypnosis (I think?)
    pub const fn compiler_summary(&self) -> &ScriptCompilerSummary {
        &self.summary
    }

    pub const fn diag_summary(&self) -> &SourceDiagnosticSummary {
        &self.diag_summary
    }

    // I think this count is WRONG because there is no consumption from budget that assumes the
    // caller is going to consume the rest.
    /// How many diagnostics were attempted to be pushed but failed due to it exceeding the budget
    pub const fn suppressed_diagnostics(&self) -> usize {
        self.diag_budget.amt_exceeded()
    }

    pub fn push_safe(&mut self, diag: SourceDiagnostic) -> bool {
        self.diag_summary.increment_from_level(diag.level);
        // Need to do something with consume() before using it so this stays checked.
        match self.diag_budget.consume(1) {
            BudgetResult::Stable | BudgetResult::LimitReached => {
                self.diag_summary.push_diag(diag);
                true
            }
            // Reward hacking my own semantics </3
            BudgetResult::Overage(_) | BudgetResult::Overflow => false,
        }
    }

    /// Consumes summary with budget safety
    ///
    /// `true` means there were no issues
    /// `false` means as many diagnostics as possible were appended, but there was an overage in budget
    pub fn merge_summary_safe(&mut self, mut other: SourceDiagnosticSummary) -> bool {
        // Smell...
        let amt = other.diags.len();

        let ok = match self.diag_budget.checked_consume(amt) {
            BudgetResult::Stable | BudgetResult::LimitReached => true,
            BudgetResult::Overage(_) => {
                other.diags.truncate(self.diag_budget.remaining());

                // Since the budget doesn't set itself to the limit the user must manually use the
                // set to limit after compensating for said limit.
                self.diag_budget.set_to_limit();
                false
            }
            BudgetResult::Overflow => return false,
        };

        self.diag_summary.merge(other);
        ok
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
                self.diag_summary.append_diags(diags);
                true
            }
            BudgetResult::Overage(_) => {
                // let can_append = amt - overage;
                // dbg!(can_append, self.diag_budget.remaining());
                // panic!("Test me");
                for i in diags.drain(..self.diag_budget.remaining()) {
                    self.diag_summary.push_diag(i);
                }
                // Since the budget doesn't set itself to the limit the user must manually use the
                // set to limit after compensating for said limit.
                self.diag_budget.set_to_limit();

                false
            }
            BudgetResult::Overflow => false,
        }
    }
}
