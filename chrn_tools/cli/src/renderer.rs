// What if the diagnostics had their min and max line cached in the given file?
// So, we'd have a struct that held the vector of diagnostics, AND the line min and max that need
// to have mapping

use std::ops::RangeInclusive;

use chrn_utils::{
    intern::Intern,
    source_map::source_diagnostic::{AnnotationKind, SourceDiagnostic},
};

// Functional or not probably doesn't matter here
#[derive(Debug)]
pub(crate) struct DiagnosticRenderer {
    diags: Vec<SourceDiagnostic>,
}

// Index would be cheaper
#[derive(Debug)]
pub(crate) struct RenderedDiagnostic<'a> {
    header: &'a str,
    kind: Vec<RenderedDiagnosticKind>,
    help: &'a [String],
    notes: &'a [String],
}

impl RenderedDiagnostic<'_> {
    pub(crate) fn new<'a>(
        header: &'a str,
        kind: Vec<RenderedDiagnosticKind>,
        // Ok maybe just attach an enum to a string that says if it's help or a note instead of an
        // entire vector?
        help: &'a [String],
        notes: &'a [String],
    ) -> RenderedDiagnostic<'a> {
        RenderedDiagnostic {
            header,
            kind,
            help,
            notes,
        }
    }
}

#[derive(Debug)]
pub(crate) enum RenderedDiagnosticKind {
    Source(RenderedSourceDiangnostic),
    Extern,
}

#[derive(Debug)]
pub(crate) struct RenderedSourceDiangnostic {
    annotation_kind: AnnotationKind,
    ln: usize,
    col: usize,
    span: RangeInclusive<usize>,
}

impl RenderedSourceDiangnostic {
    pub fn new() -> RenderedSourceDiangnostic {
        todo!()
    }
}

impl RenderedExternDiagnostic {
    pub fn new() -> RenderedSourceDiangnostic {
        todo!()
    }
}

#[derive(Debug)]
pub(crate) struct RenderedExternDiagnostic {
    annotation_kind: AnnotationKind,
}

// (Not sure about creating a system where they know what diagnostic they want from what file)
// First we should group all diagnostics together and filter them
pub(crate) fn render_cli_diags(diags: &[SourceDiagnostic] /*interner: &Intern*/) {
    let rendered_diags: Vec<RenderedDiagnostic> = Vec::new();
    for diag in diags {
        if diag.annotations.is_empty() {
            let kind = RenderedDiagnosticKind::Extern;
            // Yeah this DEFINITELY is not supposed to happen
            RenderedDiagnostic::new(&diag.core_msg, vec![kind], &diag.help, &diag.notes);
            continue;
        }
    }

    dbg!(rendered_diags);
    todo!("Render something");
}
