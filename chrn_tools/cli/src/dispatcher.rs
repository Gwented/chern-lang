use chrn_utils::{
    budget::mem_budget::MemoryBudget,
    chrn_config::ChrnConfig,
    core_error::{ConfigLoadError, ScriptError},
    source_map::source_diagnostic::Reporter,
};
use orchestration::{
    orchestrator,
    script_compiler_cache::{self},
};

use crate::{
    args::{CheckCmd, Cli, Commands, FmtCmd, GlobalArgs, QueryCmd},
    config::CliConfig,
    files, presentation, print_diags,
    renderer::{
        json_renderer,
        json_renderer::json_config::JsonRenderConfig,
        render_kind::RenderKind,
        terminal_renderer::{self, render_settings::TerminalRenderConfig},
        yaml_renderer,
        yaml_renderer::yaml_config::YamlRenderConfig,
    },
    s_ifier,
};

// Argument to nullify this should exist
/// Max diagnostics that can be held by the reporter
const MAX_DIAGNOSTICS: usize = 80;

pub fn exec(cli: &Cli, cli_cfg: &CliConfig) -> Result<String, Option<String>> {
    match &cli.command {
        Commands::Check(check_cmd) => exec_check(&check_cmd, &cli.glob_args, &cli_cfg),
        Commands::Fmt(fmt_cmd) => exec_fmt(&fmt_cmd, &cli_cfg),
        Commands::Gen(gen_cmd) => todo!(),
        Commands::Query(query_cmd) => exec_query(&query_cmd, &cli_cfg),
    }
}

/// Executes `CheckCmd` which checks for any compilation errors regarding a given Script file
fn exec_check(
    check_cmd: &CheckCmd,
    glob_args: &GlobalArgs,
    cli_cfg: &CliConfig,
) -> Result<String, Option<String>> {
    let chrn_cfg = ChrnConfig::new();
    let mut reporter = Reporter::new(MemoryBudget::new(MAX_DIAGNOSTICS));
    let path = files::make_canon(&check_cmd.path)?;
    let render_kind = RenderKind::from_check_cmd(check_cmd);

    // Please please please
    let (mut compiler, mut compiler_store, mut compiler_cache) =
        match script_compiler_cache::create_compiler_with_cache(&path, &mut reporter, chrn_cfg) {
            Ok(data) => data,
            Err(init_err) => match init_err.cfg_err {
                ConfigLoadError::Diagnostic(diag) => {
                    let footers = presentation::make_footers(&reporter);
                    let msg_opt = match render_kind {
                        RenderKind::Json => {
                            let rendered = json_renderer::render_json_diags(
                                &[diag],
                                &footers,
                                init_err.region.as_ref(),
                                &init_err.interner,
                                &JsonRenderConfig::new(check_cmd.minify),
                            );
                            println!("{rendered}");
                            None
                        }
                        RenderKind::Yaml => {
                            let rendered = yaml_renderer::render_yaml_diags(
                                &[diag],
                                &footers,
                                init_err.region.as_ref(),
                                &init_err.interner,
                                &YamlRenderConfig::new(check_cmd.minify),
                            );
                            println!("{rendered}");
                            None
                        }
                        RenderKind::Terminal => {
                            let render_cfg = TerminalRenderConfig::new(
                                glob_args.can_color,
                                cli_cfg.terminal_color_type,
                            );

                            let rendered_diags = terminal_renderer::render_terminal_diags(
                                &[diag],
                                &footers,
                                &render_cfg,
                                // reporter.budget.amt_exceeded,
                                init_err.region.as_ref(),
                                &init_err.interner,
                            );

                            print_diags!(&rendered_diags);
                            "Failed to parse configuration file".to_string().into()
                        }
                    };

                    return Err(msg_opt);
                }
                ConfigLoadError::IO(err) => {
                    let msg = format!("Process exited unsuccessfully. Reason: {err}").into();
                    return Err(msg);
                }
            },
        };

    match orchestrator::run_all(
        &mut reporter,
        &mut compiler,
        &mut compiler_store,
        Some(&mut compiler_cache),
    ) {
        Ok(_) => {
            let msg = format!("No errors found");
            Ok(msg)
        }
        Err(script_err) => match script_err {
            ScriptError::Parser(diags) | ScriptError::Semantic(diags) => {
                let footers = presentation::make_footers(&reporter);
                let msg_opt = match render_kind {
                    RenderKind::Json => {
                        let rendered = json_renderer::render_json_diags(
                            &diags,
                            &footers,
                            Some(&compiler_store.region_arena),
                            &compiler_store.interner,
                            &JsonRenderConfig::new(check_cmd.minify),
                        );
                        println!("{rendered}");
                        None
                    }
                    RenderKind::Yaml => {
                        let rendered = yaml_renderer::render_yaml_diags(
                            &diags,
                            &footers,
                            Some(&compiler_store.region_arena),
                            &compiler_store.interner,
                            &YamlRenderConfig::new(check_cmd.minify),
                        );
                        println!("{rendered}");
                        None
                    }
                    RenderKind::Terminal => {
                        let render_cfg = TerminalRenderConfig::new(
                            glob_args.can_color,
                            cli_cfg.terminal_color_type,
                        );
                        let rendered_diags = terminal_renderer::render_terminal_diags(
                            &diags,
                            &footers,
                            &render_cfg,
                            Some(&compiler_store.region_arena),
                            &compiler_store.interner,
                        );

                        //TODO: Internally cut error message strings in the parser
                        let s_suffix = s_ifier!(diags.len());
                        print_diags!(&rendered_diags);

                        format!("Reported {} error{s_suffix}", diags.len()).into()
                    }
                };

                return Err(msg_opt);
            }
            // Enforces that only one diagnostic is emitted so this is fine
            ScriptError::IO(e) => {
                let msg = format!("Process exited unsuccessfully.\nReason: {e}");
                return Err(msg.into());
            }
        },
    }

    // match chrn_manager.run_all() {
    //     Ok(_) => {
    //         let msg = format!("No errors found");
    //         Ok(msg)
    //     }
    //     Err(script_err) => match script_err {
    //         ScriptError::Parser(diags) | ScriptError::Semantic(diags) => {
    //             // For now
    //             let render_settings = RenderSettings::init();
    //             // Should print rendered diagnostics here
    //             let rendered_diags = renderer::render_cli_diags(
    //                 &diags,
    //                 &render_settings,
    //                 Some(chrn_manager.region_arena()),
    //                 chrn_manager.interner(),
    //             );
    //
    //             print_diags(&rendered_diags);
    //
    //             let sacred_s = s_ifier!(diags.len());
    //             let msg = format!("Reported {} error{sacred_s}", diags.len());
    //             return Err(msg);
    //         }
    //         ScriptError::IO(e) => {
    //             let msg = format!("Process exited unsuccessfully.\nReason: {e}");
    //             return Err(msg);
    //         }
    //     },
    // }
}

// /// Convenience function that iterates through slice and prints
// fn print_diags(rendered_diags: &[String], amt_exceeded: usize) {
//     for diag in rendered_diags {
//         eprintln!("{diag}");
//     }
//
//     if amt_exceeded > 0 {
//         // renderer's job?
//         let s_suffix = s_ifier!(amt_exceeded);
//         eprintln!("{amt_exceeded} other error{s_suffix} exist");
//     }
// }

fn exec_fmt(fmt_cmd: &FmtCmd, cli_cfg: &CliConfig) -> Result<String, Option<String>> {
    todo!("gofmt");
    // let chrn_cfg = ChrnConfig::new();
    // match formatter::fmt::fmt_script_block(&fmt_cmd.path, &settings) {
    //     Ok(_) => todo!("ok"),
    //     Err(_) => todo!("err"),
    // };
}

// Object!
fn exec_query(query_cmd: &QueryCmd, cli_cfg: &CliConfig) -> Result<String, Option<String>> {
    let chrn_cfg = ChrnConfig::new();
    let path = files::make_canon(&query_cmd.path)?;

    todo!();
    // let mut chrn_manager = match ScriptCompilerCache::new(&path, chrn_settings) {
    //     Ok(m) => m,
    //     Err(ChrnManagerInitFailure { interner, init_err }) => match init_err.cfg_err {
    //         ConfigLoadError::Diagnostic(diag) => {
    //             let render_settings =
    //                 RenderSettings::new(cli_cfg.can_color, cli_cfg.terminal_color_type);
    //             let rendered_diags =
    //                 renderer::render_cli_diags(&[diag], &render_settings, None, &interner);
    //
    //             print_diags(&rendered_diags);
    //             return Err("Failed to parse configuration file".to_string());
    //         }
    //         ConfigLoadError::IO(e) => match e.kind() {
    //             e => {
    //                 let msg = format!("Process exited unsuccessfully. Reason: {e}");
    //                 return Err(msg);
    //             }
    //         },
    //     },
    // };
    //
    // if let Err(script_err) = chrn_manager.run_all() {
    //     match script_err {
    //         ScriptError::Parser(diags) | ScriptError::Semantic(diags) => {
    //             // For now
    //             let render_settings = RenderSettings::init();
    //             // Should print rendered diagnostics here
    //             let rendered_diags = renderer::render_cli_diags(
    //                 &diags,
    //                 &render_settings,
    //                 Some(chrn_manager.region_arena()),
    //                 chrn_manager.interner(),
    //             );
    //
    //             print_diags(&rendered_diags);
    //
    //             let msg = format!("Reported {} error(s)", diags.len());
    //             return Err(msg);
    //         }
    //         ScriptError::IO(e) => {
    //             let msg = format!("Process exited unsuccessfully.\nReason: {e}");
    //             return Err(msg);
    //         }
    //     }
    // }
    //
    // // Ok...
    // // Clap commands ensure that a value must be provided if the option was chosen, so if !empty then
    // // it wasn't selected
    // let mod_opt = if !query_cmd.skip_modules.is_empty() {
    //     ModuleOptions::Skip(query_cmd.skip_modules.clone())
    // } else if !query_cmd.only_modules.is_empty() {
    //     ModuleOptions::Only(query_cmd.only_modules.clone())
    // } else if query_cmd.entry_only {
    //     ModuleOptions::EntryPoint
    // } else {
    //     ModuleOptions::All
    // };
    //
    // let chrn_cfg = DumpSettings::new(mod_opt, DumpOutputKind::Cli);
    // if let Some(ident) = &query_cmd.ident {
    //     let res =
    //         dumper::dump::dump_env(chrn_manager.compiler(), chrn_manager.interner(), &settings);
    //     dbg!(println!("{res}"));
    //     todo!("Hi")
    // } else {
    //     // dumper::symbol_printer::print_env(compiler, settings, interner)
    //     todo!("Detailing")
    // }

    todo!("detailing")
}
