use crate::args::CheckCmd;

/// RenderKind enum for choosing render output
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RenderKind {
    Terminal,
    Json,
    Yaml,
}

impl RenderKind {
    /// Gets `RenderKind` from `CheckCmd` witih bias to certain options over others if multiple are
    /// set true.
    pub(crate) fn from_check_cmd(check_cmd: &CheckCmd) -> RenderKind {
        if check_cmd.json {
            RenderKind::Json
        } else if check_cmd.yaml {
            RenderKind::Yaml
        } else {
            RenderKind::Terminal
        }
    }
}
