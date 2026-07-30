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

pub mod e0001_config_load;
pub mod e0002_safety_limits;
pub mod e0003_schema_option;
pub mod e0004_scope;
pub mod e0005_directive;
pub mod e0006_privacy;
pub mod e0007_generics;
pub mod e0008_config_decl;
pub mod e0009_import;

use chrn_utils::err_codes::{self, ErrorCode};

use crate::errors::ErrorDoc;

/// The page for one code. Exhaustive on purpose.
pub fn page(code: ErrorCode) -> ErrorDoc {
    match code {
        ErrorCode::ConfigLoadErr => e0001_config_load::page(),
        ErrorCode::CompilerSafetyLimits => e0002_safety_limits::page(),
        ErrorCode::SchemaOptionErr => e0003_schema_option::page(),
        ErrorCode::ScopeErr => e0004_scope::page(),
        ErrorCode::DirectiveErr => e0005_directive::page(),
        ErrorCode::PrivacyErr => e0006_privacy::page(),
        ErrorCode::GenericsErr => e0007_generics::page(),
        ErrorCode::ConfigDeclErr => e0008_config_decl::page(),
        ErrorCode::ImportErr => e0009_import::page(),
    }
}

/// Every page, in [`ALL_ERROR_CODES`] order. What the site generator walks.
pub fn all_pages() -> Vec<ErrorDoc> {
    err_codes::ALL_ERROR_CODES.into_iter().map(page).collect()
}
