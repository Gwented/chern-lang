pub mod html_renderer;
pub mod markdown_renderer;

use crate::doc_builder::Document;

/// Output backend for a [`Document`]. Every renderer matches exhaustively on `Node`.
pub trait Renderer {
    /// File extension the output belongs in, without the dot.
    fn extension(&self) -> &'static str;

    fn render(&self, doc: &Document) -> String;
}
