use std::borrow::Cow;

use chrn_utils::{
    chrn_config::ChrnConfig,
    core_error::{ConfigLoadError, ScriptError},
    id_types::SourceRegionId,
    intern::Intern,
    source_map::source_region::SourceRegion,
};
use compilation::{config_loader::ConfigLoader, script_compiler::reporter::Reporter};
use dumper::dump_settings::ModuleOptions;
use lang::keywords;
use orchestration::{
    constructors, orchestrator,
    script_compiler_cache::{self},
};

use crate::{
    args::{CheckCmd, Cli, Commands, EmbedCmd, FmtCmd, GlobalArgs, QueryCmd},
    config::CliConfig,
    files, presentation, print_diags,
    renderer::{
        json_renderer::{self, json_config::JsonRenderConfig},
        render_kind::RenderKind,
        terminal_renderer::{self, terminal_config::TerminalRenderConfig},
        yaml_renderer::{self, yaml_config::YamlRenderConfig},
    },
};

pub fn exec(cli: &Cli, cli_cfg: &CliConfig) -> Result<String, Option<String>> {
    match &cli.command {
        Commands::Check(check_cmd) => exec_check(&check_cmd, &cli.glob_args, &cli_cfg),
        Commands::Fmt(fmt_cmd) => exec_fmt(&fmt_cmd, &cli_cfg),
        Commands::Gen(gen_cmd) => todo!(),
        Commands::Query(query_cmd) => exec_query(&query_cmd, &cli.glob_args, &cli_cfg),
        Commands::Embed(embed_cmd) => exec_embed(&embed_cmd, &cli.glob_args, &cli_cfg),
    }
}

/// Executes `CheckCmd` which checks for any compilation errors regarding a given Script file
fn exec_check(
    check_cmd: &CheckCmd,
    glob_args: &GlobalArgs,
    cli_cfg: &CliConfig,
) -> Result<String, Option<String>> {
    // Centralized cmd to config construction for all known cmds?
    let mut builder = ChrnConfig::builder();
    if check_cmd.dbg_mode {
        builder = builder.with_logger();
    }

    let chrn_cfg = builder.build();

    let mut reporter = Reporter::new(crate::MAX_DIAGNOSTICS);
    let path = files::make_canon(&check_cmd.path)?;
    let render_kind = RenderKind::from_check_cmd(check_cmd);

    // Please please please
    let (mut compiler, mut compiler_store, mut compiler_cache) =
        match constructors::create_compiler_with_cache(&path, &mut reporter, chrn_cfg) {
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
                    let msg = format!("Process exited unsuccessfully. Reason: {err}");
                    return Err(msg.into());
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
            ScriptError::Parser | ScriptError::Semantic => {
                let footers = presentation::make_footers(&reporter);
                let msg_opt = match render_kind {
                    RenderKind::Json => {
                        let rendered = json_renderer::render_json_diags(
                            &reporter.diags,
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
                            &reporter.diags,
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
                            &reporter.diags,
                            &footers,
                            &render_cfg,
                            Some(&compiler_store.region_arena),
                            &compiler_store.interner,
                        );

                        //TODO: Internally cut error message strings in the parser
                        print_diags!(&rendered_diags);
                        // Seems redundant to have this msg
                        // "Failed to parse configuration file".to_string().into()
                        None
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
    // let chrn_cfg = ChrnConfig::default();
    // match formatter::fmt::fmt_script_block(&fmt_cmd.path, &settings) {
    //     Ok(_) => todo!("ok"),
    //     Err(_) => todo!("err"),
    // };
}

// Object!
fn exec_query(
    query_cmd: &QueryCmd,
    glob_args: &GlobalArgs,
    cli_cfg: &CliConfig,
) -> Result<String, Option<String>> {
    // Centralized cmd to config construction for all known cmds?
    let mut chrn_cfg = ChrnConfig::default();

    let mut reporter = Reporter::new(crate::MAX_DIAGNOSTICS);
    let path = files::make_canon(&query_cmd.path)?;

    // Please please please
    let (mut compiler, mut compiler_store, mut compiler_cache) =
        match constructors::create_compiler_with_cache(&path, &mut reporter, chrn_cfg) {
            Ok(data) => data,
            Err(init_err) => match init_err.cfg_err {
                ConfigLoadError::Diagnostic(diag) => {
                    let footers = presentation::make_footers(&reporter);
                    let render_cfg =
                        TerminalRenderConfig::new(glob_args.can_color, cli_cfg.terminal_color_type);

                    let rendered_diags = terminal_renderer::render_terminal_diags(
                        &[diag],
                        &footers,
                        &render_cfg,
                        // reporter.budget.amt_exceeded,
                        init_err.region.as_ref(),
                        &init_err.interner,
                    );

                    print_diags!(&rendered_diags);
                    let msg = "Failed to parse configuration file".to_string();
                    return Err(msg.into());
                }
                ConfigLoadError::IO(err) => {
                    let msg = format!("Process exited unsuccessfully. Reason: {err}");
                    return Err(msg.into());
                }
            },
        };

    match orchestrator::run_all(
        &mut reporter,
        &mut compiler,
        &mut compiler_store,
        Some(&mut compiler_cache),
    ) {
        Ok(_) => (),
        Err(script_err) => match script_err {
            ScriptError::Parser | ScriptError::Semantic => {
                let footers = presentation::make_footers(&reporter);
                let msg_opt = {
                    let render_cfg =
                        TerminalRenderConfig::new(glob_args.can_color, cli_cfg.terminal_color_type);
                    let rendered_diags = terminal_renderer::render_terminal_diags(
                        &reporter.diags,
                        &footers,
                        &render_cfg,
                        Some(&compiler_store.region_arena),
                        &compiler_store.interner,
                    );

                    //TODO: Internally cut error message strings in the parser
                    print_diags!(&rendered_diags);
                    // Seems redundant to have this msg
                    // "Failed to parse configuration file".to_string().into()
                    None
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

    // Ok...
    // Clap commands ensure that a value must be provided if the option was chosen, so if !empty then
    // it wasn't selected
    let mod_opt = if !query_cmd.skip_modules.is_empty() {
        ModuleOptions::Skip(query_cmd.skip_modules.clone())
    } else if !query_cmd.only_modules.is_empty() {
        ModuleOptions::Only(query_cmd.only_modules.clone())
    } else if query_cmd.entry_only {
        ModuleOptions::EntryPoint
    } else {
        ModuleOptions::All
    };

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

//TODO: Maybe a "-c/--check" flag that specifies that it should be checked for errs before embedding
fn exec_embed(
    embed_cmd: &EmbedCmd,
    glob_args: &GlobalArgs,
    cli_cfg: &CliConfig,
) -> Result<String, Option<String>> {
    // Centralized cmd to config construction for all known cmds?
    let mut chrn_cfg = ChrnConfig::default();

    // let mut reporter = Reporter::new(crate::MAX_DIAGNOSTICS);
    let src = files::make_canon(&embed_cmd.src_path)?;
    let dest = files::make_canon(&embed_cmd.dest_path)?;

    // Maybe allow the different ouputs for checking
    let region: SourceRegion = if embed_cmd.check {
        todo!()
    } else {
        // let handle = chrn_utils::files::file_ops::fopen(&src)?;
        // let mut interner = Intern::init();
        // let path_id = interner.intern_path(&src);
        // let region_id = SourceRegionId::new(0);
        // match ConfigLoader::new(region_id, handle, path_id, &chrn_cfg, &interner).load_config() {
        //     compilation::config_loader::ConfigLoaderOutput::Success(reg) => reg,
        //     compilation::config_loader::ConfigLoaderOutput::Broken(_, cfg_err)
        //     | compilation::config_loader::ConfigLoaderOutput::UnrecoverableErr(cfg_err) => {
        //         match cfg_err {
        //             ConfigLoadError::Diagnostic(diag) => {
        //                 let render_cfg = TerminalRenderConfig::new(
        //                     glob_args.can_color,
        //                     cli_cfg.terminal_color_type,
        //                 );
        //
        //                 let rendered_diags = terminal_renderer::render_terminal_diags(
        //                     &[diag],
        //                     &[],
        //                     &render_cfg,
        //                     region.as_ref(),
        //                     &interner,
        //                 );
        //
        //                 print_diags!(&rendered_diags);
        //                 "Failed to parse configuration file".to_string().into();
        //
        //                 return Err(msg_opt);
        //             }
        //             ConfigLoadError::IO(err) => {
        //                 let msg = format!("Process exited unsuccessfully. Reason: {err}");
        //                 return Err(msg.into());
        //             }
        //         }
        //     }
        // }
        panic!();
    };

    // The issue here is that we do NOT care about syntax, unless asked, which means we WANT
    // granular operations that don't care about semantics.
    //
    // But, we also want semantics if asked for.
    //If script start is above 0, that means there is an "@def -> @end", and if serial start is `Some`,
    // that means there exists at least an `@end`.
    //
    // Both of these mean that there doesn't need to be any insertion of an @def or @end
    let mut bytes = if region.script_start > 0 || region.serial_start.is_some() {
        Cow::Borrowed(&region.src_bytes)
    } else {
        // Wraps the src in @def[bytes]@end
        let def_end_size = keywords::ANNOTATION_CLAUSE_SIZE * 2;
        let mut altered_bytes = Vec::with_capacity(region.src_bytes.len() + def_end_size);
        altered_bytes.extend_from_slice(keywords::DEF_CLAUSE_BYTES);
        altered_bytes.extend_from_slice(&region.src_bytes);
        altered_bytes.extend_from_slice(keywords::END_CLAUSE_BYTES);
        Cow::Owned(altered_bytes)
    };
    // TODO: Eventually
    if embed_cmd.fmt {
        todo!();
    } else if embed_cmd.minify {
        todo!();
    }

    match files::write_bytes_front(&dest, &bytes) {
        Ok(_) => {
            let msg = format!(
                "Embedded\nsrc: {}\n ↓\ndest: {}",
                src.display(),
                dest.display()
            );
            Ok(msg)
        }
        Err(_) => todo!(),
    }

    // Please please please
    // let (mut compiler, mut compiler_store, mut compiler_cache) =
    //     match script_compiler_cache::create_compiler_with_cache(&src_path, &mut reporter, chrn_cfg)
    //     {
    //         Ok(data) => data,
    //         Err(init_err) => match init_err.cfg_err {
    //             ConfigLoadError::Diagnostic(diag) => {
    //                 let footers = presentation::make_footers(&reporter);
    //                 let render_cfg =
    //                     TerminalRenderConfig::new(glob_args.can_color, cli_cfg.terminal_color_type);
    //
    //                 let rendered_diags = terminal_renderer::render_terminal_diags(
    //                     &[diag],
    //                     &footers,
    //                     &render_cfg,
    //                     // reporter.budget.amt_exceeded,
    //                     init_err.region.as_ref(),
    //                     &init_err.interner,
    //                 );
    //
    //                 print_diags!(&rendered_diags);
    //                 let msg = "Failed to parse configuration file".to_string();
    //                 return Err(msg.into());
    //             }
    //             ConfigLoadError::IO(err) => {
    //                 let msg = format!("Process exited unsuccessfully. Reason: {err}");
    //                 return Err(msg.into());
    //             }
    //         },
    //     };
    //
    // match orchestrator::run_all(
    //     &mut reporter,
    //     &mut compiler,
    //     &mut compiler_store,
    //     Some(&mut compiler_cache),
    // ) {
    //     Ok(_) => (),
    //     Err(script_err) => match script_err {
    //         ScriptError::Parser | ScriptError::Semantic => {
    //             let footers = presentation::make_footers(&reporter);
    //             let msg_opt = {
    //                 let render_cfg =
    //                     TerminalRenderConfig::new(glob_args.can_color, cli_cfg.terminal_color_type);
    //                 let rendered_diags = terminal_renderer::render_terminal_diags(
    //                     &reporter.diags,
    //                     &footers,
    //                     &render_cfg,
    //                     Some(&compiler_store.region_arena),
    //                     &compiler_store.interner,
    //                 );
    //
    //                 //TODO: Internally cut error message strings in the parser
    //                 print_diags!(&rendered_diags);
    //                 // Seems redundant to have this msg
    //                 // "Failed to parse configuration file".to_string().into()
    //                 None
    //             };
    //
    //             return Err(msg_opt);
    //         }
    //         // Enforces that only one diagnostic is emitted so this is fine
    //         ScriptError::IO(e) => {
    //             let msg = format!("Process exited unsuccessfully.\nReason: {e}");
    //             return Err(msg.into());
    //         }
    //     },
    // }
}
