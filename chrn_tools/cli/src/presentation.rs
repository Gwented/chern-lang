//TEST: Not sure if this should exist but needed somewhere better than dispatcher to put footers

use chrn_utils::source_map::source_diagnostic::{Reporter, footers::FooterKind};
use compilation::script_compiler::script_compiler_summary::ScriptCompilerSummary;

// The idea behind footers is that they are from information found internally, but not made
// internally like a diagnostic would be.
//
/// Makes footers, given internal information
pub fn make_footers(reporter: &Reporter) -> Vec<FooterKind> {
    let mut footers: Vec<FooterKind> = Vec::new();
    if reporter.budget.amt_exceeded() > 0 {
        footers.push(FooterKind::DiagnosticsExceeded(
            reporter.budget.amt_exceeded() as u32,
        ));
    }

    footers
}
