#[derive(Debug)]
pub enum FooterKind {
    /// Contains amount exceeded
    DiagnosticsExceeded(u16),
    /// Contains the max module count that was exceeded
    MaxModulesExceeded(u16),
}
