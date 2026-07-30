//! The one stylesheet the site ships. Lives here rather than as a loose file so
//! `cargo run -p website` produces a complete site into any root.
//!
//! Scope is readability: measure, spacing, code blocks, light and dark. No layout system, no
//! components, no class names — the renderers emit plain tags, so this styles plain tags.

/// File name at the site root. Error pages reach it as [`ERROR_PAGE_HREF`].
pub const FILE_NAME: &str = "style.css";

/// Stylesheet href from the landing page.
pub const LANDING_HREF: &str = "style.css";

/// Stylesheet href from `errors/<label>/`.
pub const ERROR_PAGE_HREF: &str = "../../style.css";

/// Contents of [`FILE_NAME`].
pub const STYLESHEET: &str = include_str!("../site/style.css");
