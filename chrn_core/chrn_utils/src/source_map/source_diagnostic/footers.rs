#[derive(Debug)]
pub enum FooterKind {
    /// Contains amount exceeded
    DiagnosticsExceeded(u32),
}
