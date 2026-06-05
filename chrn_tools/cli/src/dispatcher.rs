use chrn_utils::{
    chrn_settings::ChrnSettings,
    core_error::{ConfigLoadError, ModuleInitError, ScriptError},
};
use orchestrator::{
    chrn_manager::{ChrnManager, ChrnManagerInitFailure},
    query,
};
use printer::symbol_printer;

use crate::{
    args::{CheckCmd, Cli, Commands, FmtCmd, QueryCmd},
    config::CliConfig,
    files,
    renderer::{self, render_settings::RenderSettings},
};

pub fn exec(cli: &Cli, cli_cfg: &CliConfig) -> Result<String, String> {
    match &cli.command {
        Commands::Check(check_cmd) => exec_check(&check_cmd, &cli_cfg),
        Commands::Fmt(fmt_cmd) => exec_fmt(&fmt_cmd, &cli_cfg),
        Commands::Gen(gen_cmd) => todo!(),
        Commands::Query(query_cmd) => exec_query(&query_cmd, &cli_cfg),
    }
}

fn exec_check(check_cmd: &CheckCmd, cli_cfg: &CliConfig) -> Result<String, String> {
    let chrn_settings = ChrnSettings::new();
    let path = files::make_canon(&check_cmd.path)?;

    let mut chrn_manager = match ChrnManager::init(&path, chrn_settings) {
        Ok(m) => m,
        // Might be going a bit too far here
        Err(ChrnManagerInitFailure { interner, init_err }) => match init_err.cfg_err {
            ConfigLoadError::General(diag) | ConfigLoadError::Module(diag) => {
                let render_settings =
                    RenderSettings::new(cli_cfg.can_color, cli_cfg.terminal_color_type);
                let rendered_diags = renderer::render_cli_diags(
                    &[diag],
                    &render_settings,
                    init_err.region.as_ref(),
                    &interner,
                );

                print_diags(&rendered_diags);

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
                // For now
                let render_settings = RenderSettings::init();
                // Should print rendered diagnostics here
                let rendered_diags = renderer::render_cli_diags(
                    &diags,
                    &render_settings,
                    Some(chrn_manager.region_arena()),
                    chrn_manager.interner(),
                );

                print_diags(&rendered_diags);

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

fn print_diags(rendered_diags: &[String]) {
    for diag in rendered_diags {
        println!("{diag}");
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
    let chrn_settings = ChrnSettings::new();
    // Right...
    let path = files::make_canon(&query_cmd.path)?;

    let mut chrn_manager = match ChrnManager::init(&path, chrn_settings) {
        Ok(m) => m,
        Err(ChrnManagerInitFailure { interner, init_err }) => match init_err.cfg_err {
            ConfigLoadError::General(diag) | ConfigLoadError::Module(diag) => {
                let render_settings =
                    RenderSettings::new(cli_cfg.can_color, cli_cfg.terminal_color_type);
                let rendered_diags =
                    renderer::render_cli_diags(&[diag], &render_settings, None, &interner);

                print_diags(&rendered_diags);

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

    if let Err(script_err) = chrn_manager.run_all() {
        match script_err {
            ScriptError::Parser(diags) | ScriptError::Semantic(diags) => {
                // For now
                let render_settings = RenderSettings::init();
                // Should print rendered diagnostics here
                let rendered_diags = renderer::render_cli_diags(
                    &diags,
                    &render_settings,
                    Some(chrn_manager.region_arena()),
                    chrn_manager.interner(),
                );

                print_diags(&rendered_diags);

                let msg = format!("Reported {} error(s)", diags.len());
                return Err(msg);
            }
            ScriptError::IO(e) => {
                let msg = format!("Process exited unsuccessfully.\nReason: {e}");
                return Err(msg);
            }
        }
    }

    let found = query::find_symbols_named(
        &chrn_manager.compiler(),
        chrn_manager.interner(),
        &query_cmd.ident,
    );

    let mut sym_strs: Vec<String> = Vec::new();
    for sym in &found {
        let sym_str =
            symbol_printer::print_symbol(chrn_manager.compiler(), sym, chrn_manager.interner());
        sym_strs.push(sym_str);
    }

    sym_strs.iter().for_each(|s| println!("{s}\n"));
    dbg!(found);
    // This should probably be in core itself, for diagnostic purposes.

    // printer::script_printer::ScriptPrinter::new(ast_info, &script_compiler);
    todo!("detailing")
}
