//! Tests for `src/doc_builder.rs`.

use crate::doc_builder::{Document, HeadingLevel, Inline, MAX_HEADING_LEVEL, Node};

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
