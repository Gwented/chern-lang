//! One module per error code. Each exposes `pub fn page() -> ErrorDoc` and owns that code's prose
//! in full — nothing here is shared or templated across codes.
//!
//! [`page`] is the only dispatch. Its match is exhaustive, so a new [`ErrorCode`] variant is a
//! compile error until a module for it exists.
//!
//! Adding a code:
//! 1. `src/pages/eNNNN_<name>.rs` with `pub fn page() -> ErrorDoc`.
//! 2. `pub mod` it below.
//! 3. Add the arm to [`page`].
//! 4. Add the variant to [`crate::errors::ALL_ERROR_CODES`] and [`crate::errors::error_title`].

pub mod e0001;
pub mod e0002;
pub mod e0003;
pub mod e0004;
pub mod e0005;
pub mod e0006;
pub mod e0007;
pub mod e0008;
pub mod e0009;

use chrn_utils::err_codes::{self, ErrorCode};

use crate::errors::ErrorDoc;

/// The page for one code. Exhaustive on purpose.
pub fn page(code: ErrorCode) -> ErrorDoc {
    match code {
        ErrorCode::ConfigLoadErr => e0001::page(),
        ErrorCode::CompilerSafetyLimits => e0002::page(),
        ErrorCode::SchemaOptionErr => e0003::page(),
        ErrorCode::ScopeErr => e0004::page(),
        ErrorCode::DirectiveErr => e0005::page(),
        ErrorCode::PrivacyErr => e0006::page(),
        ErrorCode::GenericsErr => e0007::page(),
        ErrorCode::ConfigDeclErr => e0008::page(),
        ErrorCode::ImportErr => e0009::page(),
    }
}

/// Every page, in [`ALL_ERROR_CODES`] order. What the site generator walks.
pub fn all_pages() -> Vec<ErrorDoc> {
    err_codes::ALL_ERROR_CODES.into_iter().map(page).collect()
}
