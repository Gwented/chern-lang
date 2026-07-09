//TEST: Not sure if this should exist but needed somewhere better than dispatcher to put footers

use chrn_utils::source_map::source_diagnostic::footers::FooterKind;
use compilation::script_compiler::{
    reporter::Reporter, script_compiler_summary::ScriptCompilerSummary,
};

// The idea behind footers is that they are from information found internally, but not made
// internally like a diagnostic would be.
//
/// Makes footers, given internal information
pub fn make_footers(reporter: &Reporter) -> Vec<FooterKind> {
    let mut footers: Vec<FooterKind> = Vec::new();
    let summary = reporter.summary();

    if reporter.suppressed_diagnostics() > 1 {
        footers.push(FooterKind::DiagnosticsExceeded(
            reporter.suppressed_diagnostics() as u16,
        ));
    }

    if let Some(max) = summary.exceeded_max_mods {
        footers.push(FooterKind::MaxModulesExceeded(max));
    }

    footers
}
