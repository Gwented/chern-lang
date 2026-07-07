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
    id_types::SourceRegionId,
    intern::Intern,
    source_map::{
        source_diagnostic::{DiagnosticLevel, annotations::AnnotationKind},
        source_region::SourceRegion,
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
