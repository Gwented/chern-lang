//! Tests for `src/pages.rs` — the dispatch and the invariants every page must hold.
//! Per-page content is tested in `eNNNN_*_test.rs`.

use chrn_utils::err_codes;

use crate::errors::ErrorDoc;
use crate::pages::{all_pages, page};
use crate::renderers::markdown_renderer::MarkdownRenderer;

#[test]
fn every_code_has_a_page() {
    assert_eq!(all_pages().len(), err_codes::ALL_ERROR_CODES.len());
}

#[test]
fn dispatch_returns_the_requested_code() {
    for code in err_codes::ALL_ERROR_CODES {
        assert_eq!(page(code).code(), code);
    }
}

/// A page is more than the title heading the builder emits for free.
#[test]
fn no_page_is_a_bare_title() {
    for doc in all_pages() {
        let rendered = doc.render(&MarkdownRenderer);
        assert!(
            doc.document().nodes().len() > 1,
            "{} has no body",
            doc.label()
        );
        assert!(rendered.contains(&format!("# {}", doc.label())));
    }
}

/// Sibling-error hrefs must point at codes that exist. Links elsewhere in the site (`../../`)
/// are not this test's business.
#[test]
fn cross_links_resolve() {
    let known: Vec<String> = err_codes::ALL_ERROR_CODES
        .into_iter()
        .map(ErrorDoc::sibling_href)
        .collect();

    for doc in all_pages() {
        let rendered = doc.render(&MarkdownRenderer);
        for line in rendered.lines().filter(|l| l.contains("](../E")) {
            let href = line
                .rsplit_once("](")
                .and_then(|(_, rest)| rest.split_once(')'))
                .map(|(href, _)| href.to_string())
                .unwrap_or_default();
            assert!(
                known.contains(&href),
                "{} links to unknown page {href}",
                err_codes::fmt_err_code(doc.code())
            );
        }
    }
}
