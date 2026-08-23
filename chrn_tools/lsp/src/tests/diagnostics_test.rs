use crate::analyser::{config_load_error_to_diagnostics, push_diagnostic};
use crate::tests::session::{Session, TempWorkspace};
use chrn_utils::arena::Arena;
use chrn_utils::core_error::ConfigLoadError;
use chrn_utils::id_types::{PathId, SourceRegionId};
use chrn_utils::source_map::source_diagnostic::DiagnosticLevel;
use chrn_utils::source_map::source_diagnostic::SourceDiagnostic;
use chrn_utils::source_map::source_diagnostic::annotations::AnnotationKind;
use chrn_utils::source_map::source_region::SourceRegion;
use chrn_utils::source_map::source_span::SourceSpan;
use tower_lsp::lsp_types::{DiagnosticSeverity, Position, Range};

#[test]
fn test_config_load_error_to_diagnostics_uses_absolute_positions() {
    let text = "@def\nlet x = 1\n";

    let primary_span = SourceSpan::new(SourceRegionId::new(0), 2, 6);
    let diag = SourceDiagnostic::builder(
        None,
        DiagnosticLevel::Error,
        "test error".to_string(),
        PathId::new(0),
    )
    .add_annotation(primary_span, AnnotationKind::Primary, None)
    .build();

    let cfg_err = ConfigLoadError::Diagnostic(diag);
    let lsp_diags = config_load_error_to_diagnostics(cfg_err, text, 5);

    let primary = lsp_diags
        .first()
        .expect("at least one diagnostic should be produced");

    assert_eq!(
        primary.range.start,
        Position::new(1, 2),
        "primary diagnostic must start at the absolute line/col (script_start shift applied)"
    );
    assert_eq!(
        primary.range.end,
        Position::new(1, 6),
        "primary diagnostic must end at the absolute line/col (script_start shift applied)"
    );
}

#[test]
fn test_config_load_error_to_diagnostics_no_script_start_is_identity() {
    let text = "let x = 1\n";
    let primary_span = SourceSpan::new(SourceRegionId::new(0), 4, 5);
    let diag = SourceDiagnostic::builder(
        None,
        DiagnosticLevel::Error,
        "identity test".to_string(),
        PathId::new(0),
    )
    .add_annotation(primary_span, AnnotationKind::Primary, None)
    .build();

    let cfg_err = ConfigLoadError::Diagnostic(diag);
    let lsp_diags = config_load_error_to_diagnostics(cfg_err, text, 0);

    let primary = lsp_diags.first().expect("diagnostic should be produced");
    assert_eq!(primary.range.start, Position::new(0, 4));
    assert_eq!(primary.range.end, Position::new(0, 5));
}

#[test]
fn test_config_load_error_to_diagnostics_secondary_annotation_shifted() {
    let text = "@def\nlet x = 1\n";
    let primary_span = SourceSpan::new(SourceRegionId::new(0), 0, 1);
    let secondary_span = SourceSpan::new(SourceRegionId::new(0), 4, 5);
    let diag = SourceDiagnostic::builder(
        None,
        DiagnosticLevel::Error,
        "secondary test".to_string(),
        PathId::new(0),
    )
    .add_annotation(primary_span, AnnotationKind::Primary, None)
    .add_annotation(
        secondary_span,
        AnnotationKind::Secondary,
        Some("equals sign".to_string()),
    )
    .build();

    let cfg_err = ConfigLoadError::Diagnostic(diag);
    let lsp_diags = config_load_error_to_diagnostics(cfg_err, text, 5);

    assert!(
        lsp_diags.len() >= 2,
        "primary + secondary must each produce a diagnostic"
    );

    let primary = &lsp_diags[0];
    assert_eq!(primary.range.start, Position::new(1, 0));
    assert_eq!(primary.range.end, Position::new(1, 1));

    let secondary = lsp_diags
        .iter()
        .find(|d| d.message == "equals sign" || d.message.contains("related to this"))
        .expect("secondary diagnostic must be emitted");
    assert_eq!(
        secondary.range.start,
        Position::new(1, 4),
        "secondary diagnostic must use the script_start-shifted range"
    );
    assert_eq!(secondary.range.end, Position::new(1, 5));
}

#[test]
fn test_push_diagnostic_relative_to_absolute_via_region() {
    let full_text = "@def\nlet x = 1\n";
    let main_region = SourceRegion::new(
        1,
        1,
        b"let x = 1\n".to_vec(),
        SourceRegionId::new(0),
        PathId::new(0),
        5,
        None,
    );

    let mut arena: Arena<SourceRegion, SourceRegionId> = Arena::new();
    arena.push(main_region);

    let diag = SourceDiagnostic::builder(
        None,
        DiagnosticLevel::Error,
        "type check failed".to_string(),
        PathId::new(0),
    )
    .add_annotation(
        SourceSpan::new(SourceRegionId::new(0), 0, 1),
        AnnotationKind::Primary,
        None,
    )
    .build();

    let mut lsp_diags: Vec<tower_lsp::lsp_types::Diagnostic> = Vec::new();
    push_diagnostic(
        &mut lsp_diags,
        std::slice::from_ref(&diag),
        &arena,
        full_text,
        full_text.len(),
        "chrn-typecheck",
    );

    assert_eq!(lsp_diags.len(), 1, "one diagnostic expected");
    let d = &lsp_diags[0];
    assert_eq!(
        d.range.start,
        Position::new(1, 0),
        "push_diagnostic must shift the relative span by script_start"
    );
    assert_eq!(d.range.end, Position::new(1, 1));
    assert_eq!(d.message, "type check failed");
    assert_eq!(d.source.as_deref(), Some("chrn-typecheck"));
}

#[test]
fn test_push_diagnostic_import_error_uses_importing_module_region() {
    let full_text = "@def\nimport \"missing\"\n";
    let main_region = SourceRegion::new(
        1,
        1,
        b"import \"missing\"\n".to_vec(),
        SourceRegionId::new(0),
        PathId::new(0),
        5,
        None,
    );
    let imported_region = SourceRegion::new(
        1,
        1,
        b"let unused = 0\n".to_vec(),
        SourceRegionId::new(1),
        PathId::new(1),
        0,
        None,
    );

    let mut arena: Arena<SourceRegion, SourceRegionId> = Arena::new();
    arena.push(main_region);
    arena.push(imported_region);

    let diag = SourceDiagnostic::builder(
        None,
        DiagnosticLevel::Error,
        "import not found".to_string(),
        PathId::new(0),
    )
    .add_annotation(
        SourceSpan::new(SourceRegionId::new(0), 8, 15),
        AnnotationKind::Primary,
        None,
    )
    .build();

    let mut lsp_diags: Vec<tower_lsp::lsp_types::Diagnostic> = Vec::new();
    push_diagnostic(
        &mut lsp_diags,
        std::slice::from_ref(&diag),
        &arena,
        full_text,
        full_text.len(),
        "chrn-config",
    );

    assert_eq!(lsp_diags.len(), 1, "one diagnostic expected");
    let d = &lsp_diags[0];
    assert_eq!(
        d.range.start,
        Position::new(1, 8),
        "import error must be shifted by the importing module's script_start"
    );
    assert_eq!(d.range.end, Position::new(1, 15));
    assert_eq!(d.message, "import not found");
    assert_eq!(d.source.as_deref(), Some("chrn-config"));
}

#[test]
fn test_push_diagnostic_no_matching_region_uses_fallback() {
    let full_text = "let x = 1\n";
    let main_region = SourceRegion::new(
        1,
        1,
        b"let x = 1\n".to_vec(),
        SourceRegionId::new(0),
        PathId::new(1),
        0,
        None,
    );

    let mut arena: Arena<SourceRegion, SourceRegionId> = Arena::new();
    arena.push(main_region);

    let diag = SourceDiagnostic::builder(
        None,
        DiagnosticLevel::Warn,
        "fallback test".to_string(),
        PathId::new(0),
    )
    .add_annotation(
        SourceSpan::new(SourceRegionId::new(0), 4, 5),
        AnnotationKind::Primary,
        None,
    )
    .build();

    let mut lsp_diags: Vec<tower_lsp::lsp_types::Diagnostic> = Vec::new();
    push_diagnostic(
        &mut lsp_diags,
        std::slice::from_ref(&diag),
        &arena,
        full_text,
        full_text.len(),
        "chrn-parser",
    );

    assert_eq!(lsp_diags.len(), 1);
    let d = &lsp_diags[0];
    assert_eq!(d.range.start, Position::new(0, 4));
    assert_eq!(d.range.end, Position::new(0, 5));
}

/// A document whose script lives in an embedded `@def` region must report parser
/// diagnostics at absolute file positions, not at positions relative to the region.
///
/// `let = 3` sits on line 2 of the file but line 0 of the region, so a missing
/// `script_start` addition would place the error on the data header instead.
#[tokio::test(start_paused = true)]
async fn test_session_publishes_parse_diagnostics_in_absolute_positions() {
    let workspace = TempWorkspace::new("absolute_parse_diagnostics");
    let text = "// data header\n@def\nlet = 3\n@end\ntrailing: data\n";
    let uri = workspace.write("embedded.chrn", text);

    let mut session = Session::new().await;
    let diagnostics = session.open(&uri, text).await;

    let parse_error = diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity == Some(DiagnosticSeverity::ERROR))
        .expect("`let = 3` is a parse error");

    assert_eq!(
        parse_error.source.as_deref(),
        Some("chrn-parser"),
        "parser diagnostics keep their own source"
    );
    assert_eq!(
        parse_error.range,
        Range {
            start: Position { line: 2, character: 4 },
            end: Position { line: 2, character: 5 },
        },
        "the error points at the `=` on the third line of the file"
    );
}

/// An edit that fixes the only error must publish the now-empty diagnostic set.
///
/// This drives the full debounced `did_change` path: version bump, cache invalidation,
/// re-analysis, and republication.
#[tokio::test(start_paused = true)]
async fn test_session_republishes_after_an_edit_clears_the_error() {
    let workspace = TempWorkspace::new("republish_after_fix");
    let broken = "let = 3\n";
    let uri = workspace.write("main.chrn", broken);

    let mut session = Session::new().await;
    let diagnostics = session.open(&uri, broken).await;
    assert!(
        !diagnostics.is_empty(),
        "the initial document does not parse"
    );

    session.change_full(&uri, "let value = 3\n").await;

    assert!(
        session.diagnostics(&uri).is_empty(),
        "fixing the parse error republishes an empty diagnostic set, got {:?}",
        session.diagnostics(&uri)
    );
}

/// `publish_if_current` hashes the serialized diagnostics and skips a notification when
/// the digest is unchanged, so an edit that leaves the diagnostic set identical must not
/// republish.
///
/// The digest is only observable on the backend; the wire shows nothing at all, which is
/// exactly the behaviour under test.
#[tokio::test(start_paused = true)]
async fn test_session_suppresses_a_republish_when_diagnostics_are_unchanged() {
    let workspace = TempWorkspace::new("suppressed_republish");
    let text = "let value = 3\n";
    let uri = workspace.write("main.chrn", text);

    let mut session = Session::new().await;
    session.open(&uri, text).await;

    let digest_before = session
        .backend()
        .diags_cache
        .read()
        .get(uri.as_ref())
        .copied()
        .expect("the first publish caches a digest");

    session.change_full(&uri, "let value = 4\n").await;

    let digest_after = session
        .backend()
        .diags_cache
        .read()
        .get(uri.as_ref())
        .copied()
        .expect("the digest entry survives the edit");

    assert_eq!(
        digest_before, digest_after,
        "both revisions are diagnostic-free, so nothing is republished"
    );
    assert_eq!(
        session
            .backend()
            .doc_cache
            .get_text(uri.as_ref())
            .map(|text| text.to_string())
            .as_deref(),
        Some("let value = 4\n"),
        "the edit still reached analysis despite the suppressed publish"
    );
}
