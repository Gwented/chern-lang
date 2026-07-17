#[derive(Debug)]
pub enum FooterKind {
    /// Amount of errors emitted
    ErrorsEmitted(u16),
    /// Amount of warns emitted
    WarnsEmitted(u16),
    /// Contains amount exceeded
    DiagnosticsExceeded(u16),
    /// Contains the max module count that was exceeded
    MaxModulesExceeded(u16),
}
