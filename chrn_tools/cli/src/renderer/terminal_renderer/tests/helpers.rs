use std::path::Path;

use chrn_utils::{
    arena::Arena,
    id_types::{PathId, SourceRegionId},
    intern::Intern,
    source_map::{
        line_mapping::{self, Line, LineView},
        source_diagnostic::{
            DiagnosticLevel, SourceDiagnostic,
            annotations::{Annotation, AnnotationKind},
            footers::FooterKind,
        },
        source_region::SourceRegion,
        source_span::SourceSpan,
    },
};
use common::color::TerminalColorType;

use crate::renderer::terminal_renderer::{
    render_terminal_diags, terminal_config::TerminalRenderConfig,
};

/// Every fixture here uses one region, so its id is fixed.
pub(super) const REGION: SourceRegionId = SourceRegionId::new(0);
/// Path the fixture interner registers at `PathId(0)`.
pub(super) const TEST_PATH: &str = "/test/fixture.chrn";

/// Span in the fixture region.
pub(super) fn span(start: u32, end: u32) -> SourceSpan {
    SourceSpan::new(REGION, start, end)
}

/// Span covering all of `src`, which is what the renderer maps when a diagnostic's annotations
/// reach both ends of the source.
pub(super) fn full_span(src: &str) -> SourceSpan {
    span(0, src.len() as u32)
}

pub(super) fn primary(start: u32, end: u32, label: Option<&str>) -> Annotation {
    Annotation::new(
        span(start, end),
        AnnotationKind::Primary,
        label.map(str::to_string),
    )
}

pub(super) fn secondary(start: u32, end: u32, label: Option<&str>) -> Annotation {
    Annotation::new(
        span(start, end),
        AnnotationKind::Secondary,
        label.map(str::to_string),
    )
}

/// Error diagnostic with no code, carrying `annotations` verbatim.
pub(super) fn diag(core_msg: &str, annotations: Vec<Annotation>) -> SourceDiagnostic {
    SourceDiagnostic::new(
        None,
        DiagnosticLevel::Error,
        core_msg.to_string(),
        PathId::new(0),
        annotations,
        Vec::new(),
        Vec::new(),
    )
}

/// Color is off so assertions compare plain text.
pub(super) fn plain_cfg() -> TerminalRenderConfig {
    TerminalRenderConfig::new(false, TerminalColorType::Ansi4)
}

pub(super) fn interner() -> Intern {
    let mut intern = Intern::init();
    let path_id = intern.intern_path(Path::new(TEST_PATH));
    assert_eq!(
        path_id,
        PathId::new(0),
        "fixtures assume the path interns first"
    );
    intern
}

/// Single-region arena over `src`. `abs_ln_num_start` is what `@def` extraction would record: 1
/// for a plain script file, higher when the script block starts partway into a data file.
pub(super) fn region_arena(
    src: &str,
    abs_ln_num_start: u32,
) -> Arena<SourceRegion, SourceRegionId> {
    let mut arena = Arena::new();
    let id = arena.push(SourceRegion::new(
        abs_ln_num_start,
        0,
        src.as_bytes().to_vec(),
        REGION,
        PathId::new(0),
        0,
        None,
    ));
    assert_eq!(id, REGION, "fixtures assume a single region at id 0");
    arena
}

/// Runs the full terminal renderer over one diagnostic against `src` and returns the rendered
/// text. This is the same path `chrn check` takes, minus color.
pub(super) fn render(src: &str, diag: SourceDiagnostic) -> String {
    render_at(src, diag, 1)
}

/// [`render`] with control over the region's absolute starting line.
pub(super) fn render_at(src: &str, diag: SourceDiagnostic, abs_ln_num_start: u32) -> String {
    let arena = region_arena(src, abs_ln_num_start);
    let interner = interner();
    let rendered = render_terminal_diags(&[diag], &[], Some(&arena), &interner, &plain_cfg());

    assert_eq!(rendered.len(), 1, "one diagnostic in, one rendering out");
    rendered
        .into_iter()
        .next()
        .expect("just asserted the length")
}

/// Maps `src` over `mapped` the way `render_terminal_diags` does before laying anything out.
pub(super) fn ln_view(src: &str, mapped: SourceSpan) -> LineView {
    line_mapping::form_ln_view(src.as_bytes(), &mapped)
}

/// The `Line` in `view` that holds byte `pos`.
pub(super) fn line_at(view: &LineView, pos: u32) -> &Line {
    view.lines
        .iter()
        .find(|ln| ln.ln_span.contains_part(pos))
        .unwrap_or_else(|| panic!("no mapped line holds byte {pos}"))
}

/// Strips the header and path lines so a test can assert only the annotated source block.
///
/// The renderer emits `error: <msg> \n<PATH ...>\n<body>`, so this drops the first two lines and
/// the trailing separator.
pub(super) fn body(rendered: &str) -> String {
    let lines: Vec<&str> = rendered.lines().collect();
    let start = lines
        .iter()
        .position(|ln| ln.starts_with("PATH =>"))
        .expect("every rendering with a region emits a path header")
        + 1;
    let end = lines
        .iter()
        .rposition(|ln| ln.starts_with("------"))
        .expect("every rendering ends with the visual separator");

    lines[start..end].join("\n")
}
