use std::env;

use colorc::color_type::TerminalColorType;

use crate::env_vars;

/// General information related to the CLI being used in the current session
pub struct CliConfig {
    pub terminal_color_type: TerminalColorType,
    pub env_var_repo: EnvVarRepository,
}

impl CliConfig {
    pub fn init() -> CliConfig {
        let term_type = TerminalColorType::detect();

        CliConfig {
            terminal_color_type: term_type,
            env_var_repo: Self::map_env_vars(),
        }
    }

    /// Loads all known environment variables
    fn map_env_vars() -> EnvVarRepository {
        let chrn_extensions = if let Ok(env) = env::var(env_vars::ENV_CHRN_EXTENSIONS) {
            env == "1"
        } else {
            false
        };

        EnvVarRepository { chrn_extensions }
    }
}

#[derive(Debug)]
pub struct EnvVarRepository {
    /// Whether or not "chrn-*" extensions can be searched for
    pub chrn_extensions: bool,
}
