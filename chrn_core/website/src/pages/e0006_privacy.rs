//! `E0006` Privacy error.
//TODO: Semantics not settled. Body is a placeholder.

use chrn_utils::err_codes::ErrorCode;

use crate::errors::ErrorDoc;

pub fn page() -> ErrorDoc {
    ErrorDoc::builder(ErrorCode::PrivacyErr)
        .summary("Not written yet.")
        .build()
}
