//! Static media shipped under the site root — images and video the pages point at.
//!
//! The generator does not copy files; whatever sits in `site/resources/` ships. This module only
//! spells the hrefs, which are depth-sensitive the same way [`crate::style`]'s are.

/// Directory the media lives in, relative to the site root.
pub const RESOURCES_DIR: &str = "resources";

/// Href from the landing page.
pub fn landing_href(name: &str) -> String {
    format!("{RESOURCES_DIR}/{name}")
}

/// Href from `errors/index.html`.
pub fn errors_index_href(name: &str) -> String {
    format!("../{RESOURCES_DIR}/{name}")
}

/// Href from an error page at `errors/<label>/`.
pub fn error_page_href(name: &str) -> String {
    format!("../../{RESOURCES_DIR}/{name}")
}
