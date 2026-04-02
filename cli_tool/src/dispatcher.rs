use std::{fs, io};

use common::{
    config_loader::ChernConfigLoader,
    core_error::{ConfigLoadError, CoreError, ScriptError},
    reporter,
};
use interpreter_lib::interpreter;

use crate::{
    args::{CheckCmd, Cli, Commands},
    config::CliConfig,
};

pub fn exec(cli: &Cli, cli_cfg: &CliConfig) -> Result<String, String> {
    match &cli.command {
        // Lint sub-command?
        Commands::Check(check_cmd) => process_check(&check_cmd, &cli_cfg),
        Commands::Fmt(fmt_cmd) => todo!(),
        Commands::Gen(gen_cmd) => todo!(),
    }
}

// What if this had 2 probability models?
fn process_check(check_cmd: &CheckCmd, cli_cfg: &CliConfig) -> Result<String, String> {
    let src = match fs::File::open(&check_cmd.path) {
        Ok(f) => f,
        Err(e) => match e.kind() {
            io::ErrorKind::NotFound => {
                let msg = format!("No file found in path \"{}\"", check_cmd.path.display());
                return Err(msg);
            }
            io::ErrorKind::IsADirectory => {
                let msg = format!("The path \"{}\" is a directory", check_cmd.path.display());
                return Err(msg);
            }
            io::ErrorKind::PermissionDenied => {
                // let file_name = check_cmd
                //     .path
                //     .file_name()
                //     .map(|f| f.to_os_string())
                //     .unwrap_or(OsString::from(&check_cmd.path));

                let msg = format!(
                    "The file \"{}\" does not have read permissions enabled",
                    check_cmd.path.display()
                );

                return Err(msg);
            }
            e => {
                let msg = format!("Process exited unsuccessfully.\n{e}");
                return Err(msg);
            }
        },
    };

    let mut metadata = match ChernConfigLoader::new(&check_cmd.path, src).load_config() {
        Ok(data) => data,
        Err(cfg_err) => match cfg_err {
            ConfigLoadError::Unclosed(msg) => return Err(msg),
            ConfigLoadError::IO(io_err) => match io_err.kind() {
                e => {
                    let msg = format!("Process exited unsuccessfully.\n{e}");
                    return Err(msg);
                }
            },
        },
    };

    // IS THIS BAD?
    metadata.can_color = cli_cfg.can_color;

    match interpreter::interpret_chern_cfg(&metadata) {
        Ok(_) => {
            let msg = format!("No errors found within file");
            Ok(msg)
        }
        Err(script_err) => match script_err {
            ScriptError::Parser(items) => todo!(),
            ScriptError::Semantic(items) => todo!(),
            ScriptError::IO(io_err) => todo!(),
        },
    }
}
