//! `E0002` Compiler safety limit error.
//!
//! Worked example of the page API. Prose here is a first pass the semantics are not settled.

use chrn_utils::err_codes::ErrorCode;
use chrn_utils::format_byte_size;

use crate::doc_builder::Inline;
use crate::errors::ErrorDoc;

pub fn page() -> ErrorDoc {
    // Okk but なぜ英語だけある？We're not doing that yet
    ErrorDoc::builder(ErrorCode::CompilerSafetyLimits)
        .summary("These safety limits exist to ensure security issues are addressed.\n")
        .section("Internal safety limits")
        .bullets([
            Inline::new().text(format!("Max modules = {} | Accounting for the max region size that means at most {} can be taken", chrn_utils::MAX_MODULES, format_byte_size!(chrn_utils::MAX_REGION_SIZE * chrn_utils::MAX_MODULES as usize))),
            Inline::new().text(format!("Max recursion limit = {}", chrn_utils::MAX_RECURSIVE_DEPTH)),
            // Ok but maybe a bytes conversion could exist. Didn't that already exist? Was that todol? Who is todol?
            Inline::new().text(format!("Max region size = {}", format_byte_size!(chrn_utils::MAX_REGION_SIZE))),
        ])
        .note("None of these limits are final and could be increased (Never decreased)")
        // .subsection("Not a schema error")
        .see_also([     ErrorCode::ConfigLoadErr ])
        .build()
}
// .spec_reference("../../SECURITY.md", "Safety limits")
