//! Tests for `src/renderers/html_renderer.rs`.

use crate::doc_builder::{Document, Inline, Video};
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

#[test]
fn image_renders_alt_and_title() {
    let doc = Document::builder()
        .paragraph(Inline::new().image_titled("a.png", "a \"tag\"", "hover"))
        .build();

    assert_eq!(
        render_fragment(&doc),
        "<p><img src=\"a.png\" alt=\"a &quot;tag&quot;\" title=\"hover\"></p>\n"
    );
}

#[test]
fn video_emits_flags_and_a_fallback_link() {
    let doc = Document::builder()
        .video(Video::new("clip.mp4", "demo").with_poster("clip.png"))
        .build();

    assert_eq!(
        render_fragment(&doc),
        "<video src=\"clip.mp4\" poster=\"clip.png\" controls>\n<a href=\"clip.mp4\">demo</a>\n</video>\n"
    );
}

#[test]
fn looping_clip_drops_controls() {
    let doc = Document::builder()
        .video(Video::new("clip.mp4", "demo").looping_clip())
        .build();

    assert!(
        render_fragment(&doc)
            .starts_with("<video src=\"clip.mp4\" autoplay loop muted playsinline>")
    );
}
