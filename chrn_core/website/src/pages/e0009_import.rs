//! `E0009` Import error.
//TODO: Semantics not settled. Body is a placeholder.

use chrn_utils::err_codes::ErrorCode;

use crate::errors::ErrorDoc;

pub fn page() -> ErrorDoc {
    ErrorDoc::builder(ErrorCode::ImportErr)
        .summary("Not written yet.")
        .build()
}
