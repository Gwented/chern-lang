use chrn_utils::source_map::source_diagnostic::SourceDiagnosticSummary;

#[derive(Debug)]
pub struct ConfigLoaderSummary {
    diag_summary: SourceDiagnosticSummary,
    /// Represents whether or not the read limit was reached and more of the file is left.
    /// So, if the read limit is 32KB, but the file is 33KB, the read limit being reached itself is
    /// not an error, but is suspicious which is why this is tracked.
    reached_limit_before_end: bool,
}
