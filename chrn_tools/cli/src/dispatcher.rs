use core::fmt;

use common::{
    chrn_settings::ChrnSettings,
    core_error::{ConfigLoadError, CoreError, ScriptError},
};
use interpreter_lib::interpreter;

use crate::{
    args::{CheckCmd, Cli, Commands, FmtCmd, QueryCmd},
    config::CliConfig,
};

pub fn exec(cli: &Cli, cli_cfg: &CliConfig) -> Result<String, String> {
    match &cli.command {
        Commands::Check(check_cmd) => exec_check(&check_cmd, &cli_cfg),
        Commands::Fmt(fmt_cmd) => exec_fmt(&fmt_cmd, &cli_cfg),
        Commands::Gen(gen_cmd) => todo!(),
        Commands::Query(query_cmd) => exec_query(&query_cmd, &cli_cfg),
    }
}

// What if this had 2 probability models?
fn exec_check(check_cmd: &CheckCmd, cli_cfg: &CliConfig) -> Result<String, String> {
    let settings = ChrnSettings::new(cli_cfg.can_color);

    match interpreter::interpret_chrn_cfg(&check_cmd.path, &settings) {
        Ok(_) => {
            let msg = format!("No errors found");
            Ok(msg)
        }
        Err(core_err) => match core_err {
            CoreError::Config(cfg_load_err) => match cfg_load_err {
                ConfigLoadError::General(diag) | ConfigLoadError::Module(diag) => {
                    // A bit heuristic.. but better than internally assuming a particular
                    // formatting is wanted. Probably.
                    let path = if diag.span.is_none() {
                        format!("path => \"{}\"\n", diag.path.display())
                    } else {
                        "".into()
                    };

                    eprintln!("{path}{}", diag.fmtted_diag);
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
                        eprintln!("{}", diag.fmtted_diag);
                    }

                    let msg = format!("Reported {} error(s)", diags.len());
                    return Err(msg);
                }
                ScriptError::IO(e) => {
                    let msg = format!("Process exited unsuccessfully.\nReason: {e}");
                    return Err(msg);
                }
            },
            _ => unreachable!("Serial shouldn't be checked in this command"),
        },
    }
}

fn exec_fmt(fmt_cmd: &FmtCmd, cli_cfg: &CliConfig) -> Result<String, String> {
    let settings = ChrnSettings::new(cli_cfg.can_color);
    match formatter::fmt::fmt_script_block(&fmt_cmd.path, &settings) {
        Ok(_) => todo!("ok"),
        Err(_) => todo!("err"),
    };
}

// Object!
fn exec_query(query_cmd: &QueryCmd, cli_cfg: &CliConfig) -> Result<String, String> {
    let settings = ChrnSettings::new(cli_cfg.can_color);

    match interpreter::interpret_chrn_cfg(&query_cmd.path, &settings) {
        Ok(c) => c,
        Err(core_err) => match core_err {
            CoreError::Config(cfg_load_err) => match cfg_load_err {
                ConfigLoadError::General(diag) | ConfigLoadError::Module(diag) => {
                    eprintln!("{}", diag.fmtted_diag);
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
                        eprintln!("{}", diag.fmtted_diag);
                    }

                    eprintln!("Reported {} error(s)", diags.len());

                    return Err("Failed to parse configuration file".to_string());
                }
                ScriptError::IO(e) => {
                    let msg = format!("Process exited unsuccessfully.\nReason: {e}");
                    return Err(msg);
                }
            },
            _ => unreachable!("Serial isn't checked in this command"),
        },
    };

    // printer::script_printer::ScriptPrinter::new(ast_info, &script_compiler);
    todo!("detailing")
}
