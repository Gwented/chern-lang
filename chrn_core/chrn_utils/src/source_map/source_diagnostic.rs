pub mod annotations;
pub mod footers;
// What if there was said, ".attach()" in the builder of diags to where I could tell renderers,
// do not detach or mutate this ordering in any form by file found in
use crate::{
    budget::{
        mem_budget::{BudgetResult, MemoryBudget},
        mem_cost::{self, MemoryCost},
    },
    core_error::{self, ConfigLoadError},
    id_types::PathId,
    source_map::{
        source_diagnostic::annotations::{Annotation, AnnotationKind},
        source_span::SourceSpan,
    },
};

/// This exists in case other methods or fields are considered, but is just a Vec<Diagnostic>
/// wrapper as of right now
#[derive(Debug, Default)]
pub struct Reporter {
    /// Stored diagnostics
    pub diags: Vec<SourceDiagnostic>,
    /// Maximum bytes worth of diagnostics that can be pushed before denying
    pub budget: MemoryBudget,
    // pub exceeded_max_mods: bool,
}

impl Reporter {
    pub const fn new(budget: MemoryBudget) -> Reporter {
        Reporter {
            diags: Vec::new(),
            budget,
        }
    }

    pub fn push_safe(&mut self, diag: SourceDiagnostic) -> bool {
        // Need to do something with consume() before using it so this stays checked.
        match self.budget.checked_consume(1) {
            BudgetResult::Stable | BudgetResult::LimitReached => {
                self.diags.push(diag);
                true
            }
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

        match self.budget.checked_consume(amt) {
            BudgetResult::Stable | BudgetResult::LimitReached => {
                self.diags.append(diags);
                true
            }
            BudgetResult::Overage(overage) => {
                let can_append = amt - overage;
                dbg!(can_append, self.budget.remaining());
                panic!("Test me");
                for i in diags.drain(..self.budget.remaining()) {
                    self.diags.push(i);
                }
                // Since the budget doesn't set itself to the limit the user must manually use the
                // set to limit after compensating for said limit.
                self.budget.set_to_limit();

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

// /// Although there are error types that say where the error came from, all of `CoreError` needs to
// /// still returns `Diagnostic` as a vector, which could have other areas inside of it, making this
// /// serve as persistent metadata.
// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub enum Area {
//     ConfigLoad,
//     Script,
//     Serial,
// }

/// Diagnostic intended to represent a set of instructions to be rendered.
#[derive(Debug)]
pub struct SourceDiagnostic {
    /// Severity of the given diagnostic
    pub level: DiagnosticLevel,
    /// Header message for this diagnostic
    pub core_msg: String,
    /// Stores a `PathId` instead of `SourceRegionId` because a diagnostic can exist without a region
    /// existing, such as if there was an io error before being able to read any regions.
    pub path_id: PathId,
    pub annotations: Vec<Annotation>,
    pub help: Vec<String>,
    pub notes: Vec<String>,
}

impl SourceDiagnostic {
    pub const fn new(
        level: DiagnosticLevel,
        core_msg: String,
        path_id: PathId,
        annotations: Vec<Annotation>,
        help: Vec<String>,
        notes: Vec<String>,
    ) -> SourceDiagnostic {
        SourceDiagnostic {
            level,
            core_msg,
            path_id,
            annotations,
            help,
            notes,
        }
    }

    /// Creates basic error where the given span is the primary annotation with no extra details
    pub fn basic(
        level: DiagnosticLevel,
        core_msg: String,
        path_id: PathId,
        span: SourceSpan,
    ) -> SourceDiagnostic {
        let annotations = vec![Annotation::new(span, AnnotationKind::Primary, None)];
        SourceDiagnostic {
            level,
            core_msg,
            path_id,
            annotations,
            help: Default::default(),
            notes: Default::default(),
        }
    }

    /// Creates basic error where the given span vector gives all spans a primary level annotation
    pub fn basic_multiple(
        level: DiagnosticLevel,
        core_msg: String,
        path_id: PathId,
        spans: &[SourceSpan],
    ) -> SourceDiagnostic {
        let mut annotations = Vec::new();
        for span in spans {
            annotations.push(Annotation::new(*span, AnnotationKind::Primary, None));
        }

        SourceDiagnostic {
            level,
            core_msg,
            path_id,
            annotations,
            help: Default::default(),
            notes: Default::default(),
        }
    }

    /// Creates basic error where the given span is the primary annotation with no extra details
    pub fn basic_builder(
        level: DiagnosticLevel,
        core_msg: String,
        path_id: PathId,
        span: SourceSpan,
    ) -> SourceDiagnosticBuilder {
        let annotations = vec![Annotation::new(span, AnnotationKind::Primary, None)];
        SourceDiagnosticBuilder {
            level,
            core_msg,
            path_id,
            annotations,
            help: Default::default(),
            notes: Default::default(),
        }
    }

    pub fn builder(
        level: DiagnosticLevel,
        core_msg: String,
        path_id: PathId,
    ) -> SourceDiagnosticBuilder {
        SourceDiagnosticBuilder {
            level,
            core_msg,
            path_id,
            annotations: Vec::new(),
            help: Vec::new(),
            notes: Vec::new(),
        }
    }
}

impl MemoryCost for SourceDiagnostic {
    fn cost(&self) -> usize {
        // Yes this is 1 byte and will probably never have an inner but. But um. Um.
        let level_cost = size_of::<DiagnosticLevel>();
        let core_msg_cost = mem_cost::string_cost(&self.core_msg);
        let path_id_cost = size_of::<PathId>();
        let ann_cost: usize = self.annotations.iter().map(|ann| ann.cost()).sum();

        let help_cost: usize = self
            .help
            .iter()
            .map(|help| mem_cost::string_cost(help))
            .sum();

        let notes_cost: usize = self
            .notes
            .iter()
            .map(|note| mem_cost::string_cost(note))
            .sum();

        // maybe DIAGNOSTIC_FIELDS can be asserted and enforce adding to the cost?
        // debug_assert!();
        // Ok this will definitely be checked eventually
        level_cost + core_msg_cost + path_id_cost + ann_cost + help_cost + notes_cost
    }
}

/// Optional structure that uses builder pattern to create a `SourceDiagnostic` as opposed to a regular
/// constructor
#[derive(Debug)]
pub struct SourceDiagnosticBuilder {
    level: DiagnosticLevel,
    core_msg: String,
    path_id: PathId,
    annotations: Vec<Annotation>,
    help: Vec<String>,
    notes: Vec<String>,
}

impl SourceDiagnosticBuilder {
    /// Creates annotation for the current diagnostic being built
    pub fn add_annotation(
        mut self,
        span: SourceSpan,
        kind: AnnotationKind,
        label: Option<String>,
    ) -> Self {
        let annotation = Annotation::new(span, kind, label);
        self.annotations.push(annotation);
        self
    }

    pub fn add_help(mut self, help: String) -> Self {
        self.help.push(help);
        self
    }

    pub fn add_note(mut self, note: String) -> Self {
        self.notes.push(note);
        self
    }

    pub fn build(self) -> SourceDiagnostic {
        SourceDiagnostic {
            level: self.level,
            core_msg: self.core_msg,
            path_id: self.path_id,
            annotations: self.annotations,
            help: self.help,
            notes: self.notes,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Warn,
    Note,
    Help,
}

//// Convenience function for converting a `ConfigLoadError` into a `SourceDiagnosticBuilder`
// pub fn cfg_err_to_builder(
//     cfg_err: ConfigLoadError,
//     path: &std::path::Path,
//     path_id: PathId,
// ) -> SourceDiagnosticBuilder {
//     match cfg_load_err {
//         ConfigLoadError::Diagnostic(diag) => diag,
//         ConfigLoadError::IO(io_err) => {
//             let err_str =
//                 core_error::form_string_from_io_err(&io_err, path).unwrap_or(io_err.to_string());
//             SourceDiagnostic::builder(DiagnosticLevel::Error, err_str, path)
//         }
//     }
// }
