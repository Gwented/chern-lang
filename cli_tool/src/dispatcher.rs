use std::{
    ffi::{OsStr, OsString},
    fs, io,
    path::PathBuf,
};

use common::{
    core_error::{ConfigLoadError, CoreError},
    reporter,
};
use interpreter_lib::interpreter;

use crate::{
    args::{CheckCmd, Cli, Commands},
    config::CliConfig,
};

// Anyhow?
// STOP ASKING
pub fn exec(cli: &Cli, cli_cfg: &CliConfig) -> Result<String, String> {
    match &cli.command {
        Commands::Check(check_cmd) => process_check(&check_cmd, &cli_cfg),
        Commands::FMT(fmt_cmd) => todo!(),
        Commands::Gen(gen_cmd) => todo!(),
    }
}

// What if this had a probability model?
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
                // Maybe this is bad since if the path being explicitly stated could help
                // with context this is kinda truncating it
                let file_name = check_cmd
                    .path
                    .file_name()
                    .map(|f| f.to_os_string())
                    .unwrap_or(OsString::from(&check_cmd.path));

                let msg = format!(
                    "The file \"{}\" does not have read permissions enabled",
                    file_name.display()
                );

                return Err(msg);
            }
            e => {
                let msg = format!("Process exited unsuccessfully.\n{e}");
                return Err(msg);
            }
        },
    };

    match interpreter::interpret_chrn_cfg(&src, &check_cmd.path) {
        Ok(_) => {
            let msg = format!("No errors found within file");
            Ok(msg)
        }
        Err(e) => match e {
            CoreError::Config(cfg_err) => match cfg_err {
                ConfigLoadError::UnclosedQuotes(msg) | ConfigLoadError::UnclosedDef(msg) => {
                    Err(msg)
                }
                ConfigLoadError::IO(io_err) => match io_err.kind() {
                    e => {
                        let msg = format!("Process exited unsuccessfully.\n{e}");
                        Err(msg)
                    }
                },
            },
        },
    }
}
