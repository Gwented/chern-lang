//! Tests for `src/renderers/html_renderer.rs`.

use crate::doc_builder::{Document, Inline};
use crate::renderers::Renderer;
use crate::renderers::html_renderer::{HtmlRenderer, render_fragment};

#[test]
fn heading_uses_clamped_level() {
    let doc = Document::builder().heading(99u8, "Deep").build();
    assert_eq!(render_fragment(&doc), "<h6>Deep</h6>\n");
}

#[test]
fn text_is_escaped() {
    let doc = Document::builder().paragraph("a < b & c").build();
    assert_eq!(render_fragment(&doc), "<p>a &lt; b &amp; c</p>\n");
}

#[test]
fn code_block_carries_language_class() {
    let doc = Document::builder()
        .code_block(Some("chrn".into()), "x: i32")
        .build();

    assert_eq!(
        render_fragment(&doc),
        "<pre><code class=\"language-chrn\">x: i32</code></pre>\n"
    );
}

#[test]
fn raw_bypasses_escaping() {
    let doc = Document::builder().raw("<span>ok</span>").build();
    assert_eq!(render_fragment(&doc), "<span>ok</span>\n");
}

#[test]
fn page_mode_wraps_body() {
    let doc = Document::builder()
        .paragraph(Inline::new().bold("hi"))
        .build();
    let out = HtmlRenderer::page("E0001").render(&doc);

    assert!(out.starts_with("<!DOCTYPE html>\n<html lang=\"en\">"));
    assert!(out.contains("<title>E0001</title>"));
    assert!(out.contains("<p><strong>hi</strong></p>"));
    assert!(out.ends_with("</body>\n</html>\n"));
}
