use crate::{
    budget::mem_cost::{self, MemoryCost},
    source_map::source_span::SourceSpan,
};

#[derive(Debug)]
/// Structure intended to add context to a span beyond just where to point
pub struct Annotation {
    pub span: SourceSpan,
    pub kind: AnnotationKind,
    /// Optional message like, note or, uh, um
    pub label: Option<String>,
}

impl Annotation {
    pub const fn new(span: SourceSpan, kind: AnnotationKind, label: Option<String>) -> Annotation {
        Annotation { span, kind, label }
    }
}

impl MemoryCost for Annotation {
    fn cost(&self) -> usize {
        let span_cost = size_of::<SourceSpan>();
        let kind_cost = size_of::<AnnotationKind>();
        let label_cost = if let Some(label) = &self.label {
            mem_cost::string_cost(label)
        } else {
            // Shouldn't this just be metadata size since it's a union?
            size_of::<Option<String>>()
        };
        dbg!(span_cost, kind_cost, label_cost);

        // Should this be checked?
        span_cost + kind_cost + label_cost
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
    pub const fn is_higher_priority(self, other: AnnotationKind) -> bool {
        let self_priority = self.priority();
        let other_priority = other.priority();

        self_priority > other_priority
    }

    pub const fn is_lower_priority(self, other: AnnotationKind) -> bool {
        let self_priority = self.priority();
        let other_priority = other.priority();

        self_priority < other_priority
    }

    pub fn is_eq_priority(self, other: AnnotationKind) -> bool {
        let self_priority = self.priority();
        let other_priority = other.priority();

        self_priority == other_priority
    }

    pub const fn priority(&self) -> u8 {
        match self {
            AnnotationKind::Primary => 2,
            AnnotationKind::Secondary => 1,
            AnnotationKind::Note => 0,
            AnnotationKind::Help => 0,
        }
    }
}
