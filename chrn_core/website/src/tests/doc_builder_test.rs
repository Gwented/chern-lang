//! Tests for `src/doc_builder.rs`.

use crate::doc_builder::{Document, HeadingLevel, Inline, MAX_HEADING_LEVEL, Node, Video};

#[test]
fn heading_level_clamps() {
    assert_eq!(HeadingLevel::new(0).level(), 1);
    assert_eq!(HeadingLevel::new(3).level(), 3);
    assert_eq!(HeadingLevel::new(200).level(), MAX_HEADING_LEVEL);
}

#[test]
fn str_converts_to_single_text_node() {
    let inline: Inline = "hello".into();
    assert_eq!(inline.into_nodes(), vec![Node::Text("hello".into())]);
}

#[test]
fn builder_keeps_insertion_order() {
    let doc = Document::builder()
        .heading(1u8, "Title")
        .paragraph(Inline::new().text("see ").code("i32"))
        .build();

    assert_eq!(doc.nodes().len(), 2);
    assert!(matches!(doc.nodes()[0], Node::Heading { .. }));
    assert!(matches!(doc.nodes()[1], Node::Paragraph { .. }));
}

#[test]
fn image_is_wrapped_in_a_paragraph() {
    let doc = Document::builder().image("a.png", "alt").build();

    assert!(matches!(doc.nodes()[0], Node::Paragraph { .. }));
}

#[test]
fn video_is_a_block() {
    let video = Video::new("clip.mp4", "demo");

    assert!(video.controls);
    assert!(Node::Video(video).is_block());
}

#[test]
fn looping_clip_is_silent_and_uncontrolled() {
    let clip = Video::new("clip.mp4", "demo").looping_clip();

    assert!(clip.autoplay && clip.loops && clip.muted);
    assert!(!clip.controls);
}
