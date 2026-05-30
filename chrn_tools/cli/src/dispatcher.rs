use chrn_utils::{
    chrn_settings::ChrnSettings,
    core_error::{ConfigLoadError, CoreError, ScriptError},
};
use orchestrator::chrn_manager::{self, ChrnManager};

use crate::{
    args::{CheckCmd, Cli, Commands, FmtCmd, QueryCmd},
    config::CliConfig,
    renderer,
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
    let settings = ChrnSettings::new();
    let mut chrn_manager = match ChrnManager::init(&check_cmd.path, settings) {
        Ok(m) => m,
        Err(cfg_load_err) => match cfg_load_err {
            ConfigLoadError::General(diag) | ConfigLoadError::Module(diag) => {
                dbg!(&diag);
                let rendered_diags = renderer::render_cli_diags(&[diag]);
                dbg!(rendered_diags);
                panic!();
                // This IS heurstic, but will not be changed until reported errors as a whole
                // are changed to render instead of being created in-line as they are now.
                // So, no time soon.
                //
                // Would also like a rendered or not state explicilty shown instead of raw
                // string that MIGHT be
                todo!();
                return Err("Failed to parse configuration file".to_string());
            }
            ConfigLoadError::IO(e) => match e.kind() {
                e => {
                    let msg = format!("Process exited unsuccessfully. Reason: {e}");
                    return Err(msg);
                }
            },
        },
    };

    match chrn_manager.run_all() {
        Ok(_) => {
            let msg = format!("No errors found");
            Ok(msg)
        }
        Err(script_err) => match script_err {
            ScriptError::Parser(diags) | ScriptError::Semantic(diags) => {
                todo!("Rendering not done yet");

                let msg = format!("Reported {} error(s)", diags.len());
                return Err(msg);
            }
            ScriptError::IO(e) => {
                let msg = format!("Process exited unsuccessfully.\nReason: {e}");
                return Err(msg);
            }
        },
    }
}

fn exec_fmt(fmt_cmd: &FmtCmd, cli_cfg: &CliConfig) -> Result<String, String> {
    let settings = ChrnSettings::new();
    match formatter::fmt::fmt_script_block(&fmt_cmd.path, &settings) {
        Ok(_) => todo!("ok"),
        Err(_) => todo!("err"),
    };
}

// Object!
fn exec_query(query_cmd: &QueryCmd, cli_cfg: &CliConfig) -> Result<String, String> {
    let settings = ChrnSettings::new();

    match chrn_manager::interpret_chrn_cfg(&query_cmd.path, &settings) {
        Ok(c) => c,
        Err(core_err) => match core_err {
            CoreError::Config(cfg_load_err) => match cfg_load_err {
                ConfigLoadError::General(diag) | ConfigLoadError::Module(diag) => {
                    todo!("Render err");
                    // eprintln!("{}", diag.fmtted_diag);
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
                    todo!("render err");
                    // for diag in &diags {
                    //     eprintln!("{}", diag.fmtted_diag);
                    // }

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
