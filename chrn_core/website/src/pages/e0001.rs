//! `E0001` Config load error.

use chrn_utils::err_codes::ErrorCode;

use crate::doc_builder::Inline;
use crate::errors::ErrorDoc;

pub fn page() -> ErrorDoc {
    ErrorDoc::builder(ErrorCode::ConfigLoadErr)
        .summary(
            Inline::new()
                // Using more text than needed so it's more readable formatting-wise
                // TODO: This should probably not be specific to this.
                // I CANNOT SEE
                .code(".chrn")
                .text(" files go through an initial region loading stage to ensure"),
        )
        .section(
            Inline::new()
                .text("Valid usage of ")
                .code("@def")
                .text(" and ")
                .code("@end"),
        )
        .summary("@def with @end")
        .chrn("@def\nvar->\n    x: i32\n    y: i32\n@end")
        .divider()
        .summary("Only @end")
        .chrn("let chrn = \"ch\" + \"rn\"\nlet super_chrn = \"super \" + chrn\n@end")
        .see_also([ErrorCode::CompilerSafetyLimits])
        .build()
}
