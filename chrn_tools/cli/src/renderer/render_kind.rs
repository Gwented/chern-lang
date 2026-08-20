use crate::{
    args::{CheckCmd, GlobalArgs},
    config::CliConfig,
    renderer::{
        json_renderer::json_config::JsonRenderConfig,
        terminal_renderer::terminal_config::TerminalRenderConfig,
        yaml_renderer::yaml_config::YamlRenderConfig,
    },
};

#[derive(Debug, Clone)]
pub(crate) enum RenderKind {
    Json(JsonRenderConfig),
    Terminal(TerminalRenderConfig),
    Yaml(YamlRenderConfig),
}

impl RenderKind {
    pub(crate) fn from_check_cmd(
        check_cmd: &CheckCmd,
        glob_args: &GlobalArgs,
        cli_cfg: &CliConfig,
    ) -> RenderKind {
        if check_cmd.json {
            RenderKind::Json(JsonRenderConfig::new(check_cmd.minify))
        } else if check_cmd.yaml {
            RenderKind::Yaml(YamlRenderConfig::new(check_cmd.minify))
        } else {
            RenderKind::Terminal(TerminalRenderConfig::new(
                glob_args.can_color,
                cli_cfg.terminal_color_type,
            ))
        }
    }

    pub(crate) fn to_flat(&self) -> RenderKindFlat {
        match self {
            RenderKind::Json(_) => RenderKindFlat::Json,
            RenderKind::Terminal(_) => RenderKindFlat::Terminal,
            RenderKind::Yaml(_) => RenderKindFlat::Yaml,
        }
    }
}

/// RenderKind enum for choosing render output
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RenderKindFlat {
    Terminal,
    Json,
    Yaml,
}

impl RenderKindFlat {
    /// Gets `RenderKind` from `CheckCmd` witih bias to certain options over others if multiple are
    /// set true.
    pub(crate) const fn from_check_cmd(check_cmd: &CheckCmd) -> RenderKindFlat {
        if check_cmd.json {
            RenderKindFlat::Json
        } else if check_cmd.yaml {
            RenderKindFlat::Yaml
        } else {
            RenderKindFlat::Terminal
        }
    }
}

pub(crate) enum RenderCtx {}
