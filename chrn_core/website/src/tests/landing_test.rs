//! Tests for `src/landing.rs`.

use std::path::Path;

use chrn_utils::err_codes::{self, ErrorCode};

use crate::landing::{landing, output_path, root_href};
use crate::renderers::markdown_renderer::MarkdownRenderer;

#[test]
fn links_every_code() {
    let rendered = landing().render(&MarkdownRenderer);

    for code in err_codes::ALL_ERROR_CODES {
        let label = err_codes::fmt_err_code(code);
        assert!(
            rendered.contains(&format!("](errors/{label}/)")),
            "landing page does not link {label}"
        );
    }
}

#[test]
fn hrefs_are_root_relative() {
    assert_eq!(root_href(ErrorCode::ConfigLoadErr), "errors/E0001/");
}

#[test]
fn output_path_is_the_site_root_index() {
    assert_eq!(
        output_path(Path::new("site"), &MarkdownRenderer),
        Path::new("site/index.md")
    );
}

/// Minimal by design: a title, one paragraph, one list.
#[test]
fn carries_no_prose_beyond_the_list() {
    assert_eq!(landing().nodes().len(), 3);
}
