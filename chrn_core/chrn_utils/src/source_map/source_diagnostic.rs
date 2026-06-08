// What if there was said, ".attach()" in the builder of diags to where I could tell renderers,
// do not detach or mutate this ordering in any form by file found in
use crate::{id_types::PathId, source_map::source_span::SourceSpan};

/// This exists in case other methods or fields are considered, but is just a Vec<Diagnostic>
/// wrapper as of right now
#[derive(Debug)]
pub struct Reporter {
    pub diags: Vec<SourceDiagnostic>,
}

impl Reporter {
    pub const fn new(diags: Vec<SourceDiagnostic>) -> Reporter {
        Reporter { diags }
    }
}

/// Although there are error types that say where the error came from, all of `CoreError` needs to
/// still returns `Diagnostic` as a vector, which could have other areas inside of it, making this
/// serve as persistent metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area {
    ConfigLoad,
    Script,
    Serial,
}

/// Diagnostic intended to represent a set of instructions to be rendered.
#[derive(Debug, Default)]
pub struct SourceDiagnostic {
    pub level: DiagnosticLevel,
    pub core_msg: String,
    // Stores a `PathId` instead of `SourceRegionId` because a diagnostic can exist without a region
    // existing, such as if there was an io error before being able to read any regions.
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
            ..Default::default()
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
            ..Default::default()
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

#[derive(Debug)]
/// Structure intended to add context to a span beyond just where to point
pub struct Annotation {
    pub span: SourceSpan,
    pub kind: AnnotationKind,
    /// Optional message like, note or, uh, um
    pub label: Option<String>,
}

impl Annotation {
    pub fn new(span: SourceSpan, kind: AnnotationKind, label: Option<String>) -> Annotation {
        Annotation { span, kind, label }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// Can this be replaced with DiagnosticKind?
pub enum AnnotationKind {
    /// Main part of error
    Primary,
    // Kind of help, but not help?
    /// Secondary information related to the error that could help fix it
    Secondary,
    Note,
    Help,
}

impl AnnotationKind {
    pub fn is_higher_priority(self, other: AnnotationKind) -> bool {
        let self_priority = self.priority();
        let other_priority = other.priority();

        self_priority > other_priority
    }

    pub fn is_lower_priority(self, other: AnnotationKind) -> bool {
        let self_priority = self.priority();
        let other_priority = other.priority();

        self_priority < other_priority
    }

    pub fn is_eq_priority(self, other: AnnotationKind) -> bool {
        let self_priority = self.priority();
        let other_priority = other.priority();

        self_priority == other_priority
    }

    pub fn priority(&self) -> u8 {
        match self {
            AnnotationKind::Primary => 2,
            AnnotationKind::Secondary => 1,
            AnnotationKind::Note => 0,
            AnnotationKind::Help => 0,
        }
    }
}

// pub enum PointerKind {
//     Carot,
//     Hyphen,
//     Tilde,
//     Plus,
// }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Warn,
    Note,
    Help,
}

impl Default for DiagnosticLevel {
    fn default() -> Self {
        DiagnosticLevel::Error
    }
}
