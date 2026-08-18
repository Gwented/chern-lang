//TEST: Not sure if this should exist but needed somewhere better than dispatcher to put footers

use chrn_utils::source_map::source_diagnostic::footers::FooterKind;
use compilation::script_compiler::reporter::Reporter;

// The ordering of each footer insertion is on purpose

// The idea behind footers is that they are from information found internally, but not always
// made internally.

/// Makes footers, given internal information
pub fn make_footers(reporter: &Reporter) -> Vec<FooterKind> {
    let mut footers: Vec<FooterKind> = Vec::new();
    let comp_summary = reporter.compiler_summary();
    let diag_summary = reporter.diag_summary();

    if reporter.suppressed_diagnostics() > 0 {
        footers.push(FooterKind::DiagnosticsExceeded(
            reporter.suppressed_diagnostics() as u16,
        ));
    }

    if let Some(max) = comp_summary.exceeded_max_mods {
        footers.push(FooterKind::MaxModulesExceeded(max));
    }

    if diag_summary.warn_count() > 0 {
        footers.push(FooterKind::WarnsEmitted(diag_summary.warn_count()));
    }

    if diag_summary.err_count() > 0 {
        footers.push(FooterKind::ErrorsEmitted(diag_summary.err_count()));
    }

    footers
}
