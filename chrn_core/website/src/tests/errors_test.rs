//! Tests for `src/errors.rs`.

use std::path::Path;

use chrn_utils::err_codes::{self, ErrorCode};

use crate::errors::{ErrorDoc, error_title};
use crate::renderers::markdown_renderer::MarkdownRenderer;

#[test]
fn labels_are_zero_padded() {
    assert_eq!(err_codes::fmt_err_code(ErrorCode::ConfigLoadErr), "E0001");
    assert_eq!(err_codes::fmt_err_code(ErrorCode::ImportErr), "E0009");
}

#[test]
fn every_code_has_a_title() {
    for code in err_codes::ALL_ERROR_CODES {
        assert!(!error_title(code).is_empty());
    }
}

#[test]
fn title_heading_is_emitted_first() {
    let doc = ErrorDoc::builder(ErrorCode::ScopeErr).build();

    assert_eq!(doc.render(&MarkdownRenderer), "# E0004: Scope error\n");
    assert_eq!(doc.label(), "E0004");
}

#[test]
fn presets_render_sections() {
    let doc = ErrorDoc::builder(ErrorCode::GenericsErr)
        .summary("Users cannot declare generics.")
        .wrong_example("struct Thing<T> { x: T }")
        .see_also([ErrorCode::ScopeErr])
        .build();

    let rendered = doc.render(&MarkdownRenderer);

    assert!(rendered.contains("# E0007: Generics error"));
    assert!(rendered.contains("## Wrong"));
    assert!(rendered.contains("```chrn\nstruct Thing<T> { x: T }\n```"));
    assert!(rendered.contains("- [`E0004`: Scope error](../E0004/)"));
}

#[test]
fn output_path_is_per_code_directory() {
    let doc = ErrorDoc::builder(ErrorCode::ImportErr).build();
    let path = doc.output_path(Path::new("site"), &MarkdownRenderer);

    assert_eq!(path, Path::new("site/errors/E0009/index.md"));
}
