//! Tests for `src/landing.rs`.

use std::path::Path;

use crate::landing::{landing, output_path, sections};
use crate::renderers::markdown_renderer::MarkdownRenderer;

#[test]
fn links_every_section() {
    let rendered = landing().render(&MarkdownRenderer);

    for section in sections() {
        assert!(
            rendered.contains(&format!("]({})", section.href)),
            "landing page does not link {}",
            section.name
        );
    }
}

#[test]
fn links_the_error_index_not_individual_codes() {
    let rendered = landing().render(&MarkdownRenderer);

    assert!(rendered.contains("](errors/)"));
    assert!(!rendered.contains("errors/E0001/"));
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
