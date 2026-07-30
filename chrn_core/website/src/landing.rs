//! The site root page. One link per error code, nothing else.
//!
//! Sits beside [`crate::errors`] rather than under [`crate::pages`]: it is not an [`ErrorDoc`],
//! and it owns no prose about any individual code — titles come from
//! [`crate::errors::error_title`], so a new code appears here for free.

use std::path::{Path, PathBuf};

use chrn_utils::err_codes::{self, ErrorCode};

use crate::doc_builder::{Document, Inline};
use crate::errors::{ERRORS_DIR, error_title};
use crate::renderers::Renderer;

/// Href of an error page from the site root.
pub fn root_href(code: ErrorCode) -> String {
    format!("{ERRORS_DIR}/{}/", err_codes::fmt_err_code(code))
}

/// Path of the landing page under a site root, e.g. `site/index.html`.
pub fn output_path<R: Renderer>(root: &Path, renderer: &R) -> PathBuf {
    root.join(format!("index.{}", renderer.extension()))
}

/// The index: title, one line of orientation, one link per code in [`ALL_ERROR_CODES`] order.
pub fn landing() -> Document {
    // Creates bullet points with links to all error codes
    let err_code_bullets = err_codes::ALL_ERROR_CODES.into_iter().map(|code| {
        Inline::new().link(
            root_href(code),
            Inline::new()
                .code(err_codes::fmt_err_code(code))
                .text(format!(" {}", error_title(code))),
        )
    });

    Document::builder()
        .heading(1, "chrn error codes")
        .paragraph(
            Inline::new()
                .text("Every code the compiler emits. Codes are category-level, so one page covers a range of diagnostics."),
        )
        .bullets(err_code_bullets)
        .build()
}
