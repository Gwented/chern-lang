use crate::analyser::{config_load_error_to_diagnostics, push_diagnostics};
use chrn_utils::arena::Arena;
use chrn_utils::core_error::ConfigLoadError;
use chrn_utils::id_types::{PathId, SourceRegionId};
use chrn_utils::source_map::source_diagnostic::DiagnosticLevel;
use chrn_utils::source_map::source_diagnostic::SourceDiagnostic;
use chrn_utils::source_map::source_diagnostic::annotations::AnnotationKind;
use chrn_utils::source_map::source_region::SourceRegion;
use chrn_utils::source_map::source_span::SourceSpan;
use tower_lsp::lsp_types::Position;

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
fn test_push_diagnostics_relative_to_absolute_via_region() {
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
    push_diagnostics(
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
        "push_diagnostics must shift the relative span by script_start"
    );
    assert_eq!(d.range.end, Position::new(1, 1));
    assert_eq!(d.message, "type check failed");
    assert_eq!(d.source.as_deref(), Some("chrn-typecheck"));
}

#[test]
fn test_push_diagnostics_import_error_uses_importing_module_region() {
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
    push_diagnostics(
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
fn test_push_diagnostics_no_matching_region_uses_fallback() {
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
    push_diagnostics(
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
