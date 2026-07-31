//! Tests for `src/renderers/markdown_renderer.rs`.

use crate::doc_builder::{Document, Inline, Video};
use crate::renderers::markdown_renderer::render;

#[test]
fn heading_has_space_and_blank_line() {
    let doc = Document::builder()
        .heading(2u8, "Title")
        .paragraph("body")
        .build();

    assert_eq!(render(&doc), "## Title\n\nbody\n");
}

#[test]
fn code_block_fence_outgrows_content() {
    let doc = Document::builder()
        .code_block(Some("chrn".into()), "a ``` b")
        .build();

    assert_eq!(render(&doc), "````chrn\na ``` b\n````\n");
}

#[test]
fn list_items_render_per_kind() {
    let doc = Document::builder()
        .bullets(["one", "two"])
        .numbered(["one"])
        .build();

    assert_eq!(render(&doc), "- one\n- two\n\n1. one\n");
}

#[test]
fn inline_composition() {
    let doc = Document::builder()
        .paragraph(Inline::new().text("see ").code("i32").text(" now"))
        .build();

    assert_eq!(render(&doc), "see `i32` now\n");
}

#[test]
fn image_renders_as_a_bang_link() {
    let doc = Document::builder().image("a.png", "a diagram").build();

    assert_eq!(render(&doc), "![a diagram](a.png)\n");
}

/// Markdown has no video element, so it degrades to a link.
#[test]
fn video_degrades_to_a_link() {
    let doc = Document::builder()
        .captioned_video(Video::new("clip.mp4", "demo"), "what it does")
        .build();

    assert_eq!(render(&doc), "[demo](clip.mp4)\n\n*what it does*\n");
}
