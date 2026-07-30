//! Small string-mapping helpers shared by the structured (non-terminal)
//! renderers. Centralizes how diagnostic field values are spelled out as
//! plain strings and how a `SourceRegionId` is resolved to a path, so a
//! new `DiagnosticLevel` or `AnnotationKind` variant only needs to be wired
//! up in one place.
//!
//! These helpers are format-agnostic on purpose: each structured renderer
//! is free to wrap the resulting strings in JSON, YAML, or anything else
//! using its own escape module.

use chrn_utils::{
    arena::Arena,
    err_codes,
    id_types::SourceRegionId,
    intern::Intern,
    source_map::{
        source_diagnostic::{DiagnosticLevel, annotations::AnnotationKind},
        source_region::SourceRegion,
        source_span::SourceSpan,
    },
};

/// Plain string name used for a diagnostic level.
///
/// Lowercase, matches the convention used by every structured renderer
/// (JSON, YAML, and any future ones) so consumers can rely on a single
/// vocabulary for `level`.
pub(super) fn level_str(level: DiagnosticLevel) -> &'static str {
    match level {
        DiagnosticLevel::Error => "error",
        DiagnosticLevel::Warn => "warn",
        DiagnosticLevel::Note => "note",
        DiagnosticLevel::Help => "help",
    }
}

/// Plain string name used for an annotation kind.
///
/// Lowercase, same rationale as [`level_str`].
pub(super) fn annotation_kind_str(kind: AnnotationKind) -> &'static str {
    match kind {
        AnnotationKind::Primary => "primary",
        AnnotationKind::Secondary => "secondary",
        AnnotationKind::Note => "note",
        AnnotationKind::Help => "help",
    }
}

/// Resolves a `SourceRegionId` to its path string when both the region
/// arena is available and the region is present in it.
///
/// Returns `None` for either "no arena" or "region not found", which lets
/// callers collapse both cases into a single "emit `null`" branch.
pub(super) fn resolve_region_path(
    region_arena_opt: Option<&Arena<SourceRegion, SourceRegionId>>,
    interner: &Intern,
    region_id: SourceRegionId,
) -> Option<String> {
    let arena = region_arena_opt?;
    let region = arena.get(region_id)?;
    Some(interner.search_path(region.path_id).display().to_string())
}

/// Projects a span's `start`/`end` from region-relative offsets to
/// absolute file offsets using the owning region's `script_start`.
///
/// The rest of the pipeline (lexing, parsing, semantic analysis) emits
/// `SourceSpan` values whose `start`/`end` are byte offsets into the
/// owning `SourceRegion::src_bytes`, not the underlying file. This is
/// deliberate: the loader only retains the script portion of the file
/// in `src_bytes`, so a relative position is the only thing that is
/// stable across the pipeline. External tooling, however, has no way
/// to recover the `script_start` it needs to translate those relative
/// positions back into the file the user actually edited.
///
/// The structured (JSON, YAML) renderers run at the end of the pipeline
/// where the region arena is available, so they take on that projection
/// themselves: they add the region's `script_start` to the relative
/// `start`/`end` and emit absolute byte offsets that a tool can map
/// directly onto the source file.
///
/// When the region arena is missing or the region id is not present in
/// it, the raw relative values are returned unchanged. The structured
/// renderers still emit a usable, well-formed document in that case —
/// they just cannot guarantee the offsets are absolute.
pub(super) fn project_absolute_span(
    region_arena_opt: Option<&Arena<SourceRegion, SourceRegionId>>,
    span: &SourceSpan,
) -> (u32, u32) {
    let script_start = region_arena_opt
        .and_then(|arena| arena.get(span.region_id))
        .map_or(0, |region| region.script_start as u32);
    (span.start + script_start, span.end + script_start)
}
