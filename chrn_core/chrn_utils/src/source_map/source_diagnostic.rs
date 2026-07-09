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
