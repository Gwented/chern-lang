//! `E0008` Config declaration error.
//TODO: Semantics not settled. Body is a placeholder.

use chrn_utils::err_codes::ErrorCode;

use crate::errors::ErrorDoc;

pub fn page() -> ErrorDoc {
    ErrorDoc::builder(ErrorCode::ConfigDeclErr)
        .summary("Not written yet.")
        .build()
}
