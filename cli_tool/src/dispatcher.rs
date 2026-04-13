use common::{
    chern_settings::ChernSettings,
    core_error::{ConfigLoadError, CoreError, ScriptError},
};
use interpreter_lib::interpreter;

use crate::{
    args::{CheckCmd, Cli, Commands},
    config::CliConfig,
};

pub fn exec(cli: &Cli, cli_cfg: &CliConfig) -> Result<String, String> {
    match &cli.command {
        // Lint sub-command? eslint
        Commands::Check(check_cmd) => exec_check(&check_cmd, &cli_cfg),
        Commands::Fmt(fmt_cmd) => todo!(),
        Commands::Gen(gen_cmd) => todo!(),
    }
}

// What if this had 2 probability models?
fn exec_check(check_cmd: &CheckCmd, cli_cfg: &CliConfig) -> Result<String, String> {
    let settings = ChernSettings::new(cli_cfg.can_color);

    match interpreter::interpret_chern_cfg(&check_cmd.path, &settings) {
        Ok(_) => {
            let msg = format!("No errors found within file");
            Ok(msg)
        }
        Err(core_err) => match core_err {
            CoreError::Config(cfg_load_err) => match cfg_load_err {
                ConfigLoadError::Unclosed(msg) | ConfigLoadError::Module(msg) => {
                    eprintln!("{msg}");
                    return Err("Failed to parse configuration file".to_string());
                }
                ConfigLoadError::IO(e) => match e.kind() {
                    e => {
                        let msg = format!("Process exited unsuccessfully. Reason: {e}");
                        return Err(msg);
                    }
                },
            },
            CoreError::Script(script_err) => match script_err {
                ScriptError::Parser(diags) | ScriptError::Semantic(diags) => {
                    for diag in &diags {
                        eprintln!("{}", diag.msg);
                    }

                    eprintln!("Reported {} error(s)", diags.len());

                    return Err("Failed to parse configuration file".to_string());
                }
                ScriptError::IO(e) => {
                    let msg = format!("Process exited unsuccessfully. Reason: {e}");
                    return Err(msg);
                }
            },
            _ => unreachable!(),
        },
    }
}
