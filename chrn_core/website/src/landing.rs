//! The site root page. A hub: one entry per section, no content of its own.
//!
//! Owns no prose about any error code — the code listing is its own page, built by
//! [`crate::errors::index`]. A section appears here by being added to [`sections`].

use std::path::{Path, PathBuf};

use crate::doc_builder::{Document, Inline};
use crate::errors;
use crate::renderers::Renderer;

/// One linked section of the site.
pub struct Section {
    /// Href from the site root.
    pub href: String,
    pub name: &'static str,
    /// One line describing what the section covers.
    pub blurb: &'static str,
}

/// Every section the landing links, in listed order.
pub fn sections() -> Vec<Section> {
    vec![Section {
        href: errors::index_root_href(),
        name: "Error codes",
        blurb: "every code the compiler emits, one page per category",
    }]
}

/// Path of the landing page under a site root, e.g. `site/index.html`.
pub fn output_path<R: Renderer>(root: &Path, renderer: &R) -> PathBuf {
    root.join(format!("index.{}", renderer.extension()))
}

/// The hub: title, one line of orientation, one link per section.
pub fn landing() -> Document {
    let section_bullets = sections().into_iter().map(|section| {
        Inline::new()
            .link(section.href, Inline::new().bold(section.name))
            .text(format!(" - {}", section.blurb))
    });

    Document::builder()
        .heading(1, "chrn")
        .paragraph(
            Inline::new().text(
                "A typed scripting language for altering schemas for cross-language serialization config.",
            ),
        )
        .bullets(section_bullets)
        .build()
}
