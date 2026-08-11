pub mod annotations;
pub mod footers;
// What if there was said, ".attach()" in the builder of diags to where I could tell renderers,
// do not detach or mutate this ordering in any form by file found in
use crate::{
    budget::mem_cost::{self, MemoryCost},
    err_codes::ErrorCode,
    id_types::PathId,
    source_map::{
        source_diagnostic::annotations::{Annotation, AnnotationKind},
        source_span::SourceSpan,
    },
    utils::SharedU32,
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
    pub err_code: Option<ErrorCode>,
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
        err_code: Option<ErrorCode>,
        level: DiagnosticLevel,
        core_msg: String,
        path_id: PathId,
        annotations: Vec<Annotation>,
        help: Vec<String>,
        notes: Vec<String>,
    ) -> SourceDiagnostic {
        SourceDiagnostic {
            err_code,
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
        err_code: Option<ErrorCode>,
        level: DiagnosticLevel,
        core_msg: String,
        path_id: PathId,
        span: SourceSpan,
    ) -> SourceDiagnostic {
        let annotations = vec![Annotation::new(span, AnnotationKind::Primary, None)];
        SourceDiagnostic {
            err_code,
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
        err_code: Option<ErrorCode>,
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
            err_code,
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
        err_code: Option<ErrorCode>,
        level: DiagnosticLevel,
        core_msg: String,
        path_id: PathId,
        span: SourceSpan,
    ) -> SourceDiagnosticBuilder {
        let annotations = vec![Annotation::new(span, AnnotationKind::Primary, None)];
        SourceDiagnosticBuilder {
            err_code,
            level,
            core_msg,
            path_id,
            annotations,
            help: Default::default(),
            notes: Default::default(),
        }
    }

    pub fn builder<S: Into<String>>(
        err_code: Option<ErrorCode>,
        level: DiagnosticLevel,
        core_msg: S,
        path_id: PathId,
    ) -> SourceDiagnosticBuilder {
        SourceDiagnosticBuilder {
            err_code,
            level,
            core_msg: core_msg.into(),
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
    err_code: Option<ErrorCode>,
    level: DiagnosticLevel,
    core_msg: String,
    path_id: PathId,
    annotations: Vec<Annotation>,
    help: Vec<String>,
    notes: Vec<String>,
}

impl SourceDiagnosticBuilder {
    pub fn set_core_msg(mut self, core_msg: String) -> Self {
        self.core_msg = core_msg;
        self
    }

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

    pub fn add_help<S: Into<String>>(mut self, help: S) -> Self {
        self.help.push(help.into());
        self
    }

    pub fn add_note<S: Into<String>>(mut self, note: S) -> Self {
        self.notes.push(note.into());
        self
    }
    pub fn set_error_code(mut self, code: ErrorCode) -> Self {
        self.err_code = Some(code);
        self
    }

    pub fn build(self) -> SourceDiagnostic {
        SourceDiagnostic {
            err_code: self.err_code,
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
    // May remove since these are not meant for top level
    Note,
    Help,
}

// There's something about getters and setters for generic APIs that make sense.
// Welcome back Java. (Rude)

// Budget?
// I feel like we really want budget here
// We'll just keep a higher abstraction for budget handling for now
/// Generic structure for providing a reporting summary
#[derive(Debug, Default)]
pub struct SourceDiagnosticSummary {
    /// Left is warn, right is err
    warn_and_err_count: SharedU32,
    // We lost.
    pub diags: Vec<SourceDiagnostic>,
    // Ok but what if we had SharedSignalU32 where it was u31
    // PADDING ITS PADDING
    /// Whether or not the errors that have occurred are terminal
    is_terminal: bool,
}

//TEST: The extraction of data is painful, but trying to give encapsulation a fair chance on a real
//structure
impl SourceDiagnosticSummary {
    pub const fn new(warn_and_err_count: SharedU32, is_terminal: bool) -> SourceDiagnosticSummary {
        SourceDiagnosticSummary {
            warn_and_err_count,
            diags: Vec::new(),
            is_terminal,
        }
    }

    /// Internally checks the kind of the diagnostic before pushing to keep count
    pub fn push_diag(&mut self, diag: SourceDiagnostic) {
        match diag.level {
            DiagnosticLevel::Error => self.increment_err(),
            DiagnosticLevel::Warn => self.increment_warn(),
            // We don't emit these and may remove them as top-level kinds
            DiagnosticLevel::Help | DiagnosticLevel::Note => (),
        };
        self.diags.push(diag);
    }

    pub fn append_diags(&mut self, diags: &mut Vec<SourceDiagnostic>) {
        for diag in diags.iter() {
            self.increment_from_level(diag.level);
        }
        self.diags.append(diags);
    }
    // Boolean on whether or not to accept the terminality?
    /// Terminalness does NOT carry over because summaries operate under a different context.
    /// If that is desired then externally do so.
    pub fn merge(&mut self, mut other: SourceDiagnosticSummary) {
        self.diags.append(&mut other.diags);
        // I don't know about this. Operator overloading is a little too transient
        self.warn_and_err_count += other.warn_and_err_count;
    }

    /// Takes data from summary without transferring ownership
    ///
    /// Sets other's values to zero where possible, but does not touch is_terminal.
    ///
    /// Terminalness does NOT carry over because summaries operate under a different context.
    /// If that is desired then externally do so.
    pub fn append_summary(&mut self, other: &mut SourceDiagnosticSummary) {
        self.diags.append(&mut other.diags);

        self.warn_and_err_count += other.warn_and_err_count;
        other.warn_and_err_count.set_shared_inner(0);
    }

    pub const fn increment_from_level(&mut self, level: DiagnosticLevel) {
        match level {
            DiagnosticLevel::Error => self.increment_err(),
            DiagnosticLevel::Warn => self.increment_warn(),
            // We don't emit these and may remove them as top-level kinds
            DiagnosticLevel::Help | DiagnosticLevel::Note => (),
        };
    }

    pub const fn set_terminal(&mut self, is_terminal: bool) {
        self.is_terminal = is_terminal;
    }

    pub const fn err_count(&self) -> u16 {
        self.warn_and_err_count.right()
    }

    pub const fn warn_count(&self) -> u16 {
        self.warn_and_err_count.left()
    }

    pub const fn increment_warn(&mut self) {
        self.warn_and_err_count.add_left(1);
    }

    pub const fn increment_err(&mut self) {
        self.warn_and_err_count.add_right(1);
    }

    pub const fn add_warn(&mut self, amt: u16) {
        self.warn_and_err_count.add_left(amt);
    }

    pub const fn add_err(&mut self, amt: u16) {
        self.warn_and_err_count.add_right(amt);
    }

    pub const fn diags(&self) -> &Vec<SourceDiagnostic> {
        &self.diags
    }

    pub const fn is_terminal(&self) -> bool {
        self.is_terminal
    }
}

// Wow. This is...wow.
// Beautiful
/// Allows for sink implementors to push or append while performing internal computations if needed.
pub trait SourceDiagnosticSink {
    fn push_diagnostic(&mut self, diag: SourceDiagnostic);
    fn append_diagnostics(&mut self, diags: &mut Vec<SourceDiagnostic>);
}

impl SourceDiagnosticSink for SourceDiagnosticSummary {
    fn push_diagnostic(&mut self, diag: SourceDiagnostic) {
        self.push_diag(diag);
    }

    fn append_diagnostics(&mut self, diags: &mut Vec<SourceDiagnostic>) {
        self.append_diags(diags);
    }
}

impl SourceDiagnosticSink for Vec<SourceDiagnostic> {
    fn push_diagnostic(&mut self, diag: SourceDiagnostic) {
        self.push(diag);
    }

    fn append_diagnostics(&mut self, diags: &mut Vec<SourceDiagnostic>) {
        self.append(diags);
    }
}
