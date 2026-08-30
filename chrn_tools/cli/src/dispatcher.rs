use std::borrow::Cow;

use chrn_utils::{
    chrn_config::ChrnConfig,
    core_error::{ConfigLoadError, ScriptError},
    files::file_ops,
    id_types::SourceRegionId,
    source_map::source_region::SourceRegion,
};
use compilation::{
    module::{self, module_concepts::ModuleState},
    script_compiler::reporter::Reporter,
};
use dumper::dump_settings::ModuleOptions;
use lang::keywords;
use macrosc::byte_formatter;
use orchestration::{constructors, orchestrator};

use crate::{
    args::{CheckCmd, Cli, Commands, DocCmd, EmbedCmd, FmtCmd, GlobalArgs, QueryCmd},
    config::CliConfig,
    files, presentation, print_diags,
    renderer::{
        self,
        render_kind::{RenderKind, RenderKindFlat},
        terminal_renderer::{self, terminal_config::TerminalRenderConfig},
    },
};

pub fn exec(cli: &Cli, cli_cfg: &CliConfig) -> Result<String, Option<String>> {
    match &cli.command {
        Commands::Check(check_cmd) => exec_check(check_cmd, &cli.glob_args, &cli_cfg),
        Commands::Fmt(fmt_cmd) => exec_fmt(fmt_cmd, &cli_cfg),
        Commands::Gen(gen_cmd) => todo!(),
        Commands::Query(query_cmd) => exec_query(query_cmd, &cli.glob_args, &cli_cfg),
        Commands::Embed(embed_cmd) => exec_embed(embed_cmd, &cli.glob_args, &cli_cfg),
        Commands::Doc(doc_cmd) => exec_doc(doc_cmd, &cli.glob_args, cli_cfg),
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
        builder = builder.add_logger();
    }
    if check_cmd.perf_tracker {
        builder = builder.add_perf_tracker();
    }

    let chrn_cfg = builder.build();

    let mut reporter = Reporter::new(crate::MAX_DIAGNOSTICS);
    let path = files::make_canon(&check_cmd.path)?;
    let src = match file_ops::fopen(&path) {
        Ok(f) => f,
        Err((_, err_msg)) => {
            let msg = format!("Process exited unsuccessfully.\nReason: {err_msg}");
            return Err(msg.into());
        }
    };

    let render_kind = RenderKind::from_check_cmd(check_cmd, glob_args, cli_cfg);

    // Please please please
    let (mut compiler, mut compiler_store, mut compiler_cache) =
        match constructors::create_compiler_with_cache(&path, src, &mut reporter, chrn_cfg) {
            Ok(data) => data,
            Err(init_err) => {
                let msg_opt = match init_err.cfg_err {
                    ConfigLoadError::Diagnostic(diag) => {
                        reporter.push_safe(diag);
                        match render_kind.to_flat() {
                            RenderKindFlat::Json | RenderKindFlat::Yaml => None,
                            RenderKindFlat::Terminal => {
                                "Failed to parse config file".to_string().into()
                            }
                        }
                    }
                    ConfigLoadError::IO(err) => {
                        let msg = format!("Process exited unsuccessfully.\nReason: {err}");
                        msg.into()
                    }
                };

                let footers = presentation::make_footers(&reporter);
                let rendered = renderer::render(
                    reporter.diag_summary().diags(),
                    &footers,
                    init_err.region.as_ref(),
                    &init_err.interner,
                    &render_kind,
                );

                eprintln!("{rendered}");
                return Err(msg_opt);
            }
        };

    let msg_res = match orchestrator::run_all(
        &mut reporter,
        &mut compiler,
        &mut compiler_store,
        Some(&mut compiler_cache),
    ) {
        // Yes. All green. Clean, even.
        // Is this redundant? Kinda just want it to say "complete" no "complete:"
        Ok(_) => Ok("No errors".to_string()),
        Err(script_err) => match script_err {
            ScriptError::Parser | ScriptError::Semantic => Err(None),
            ScriptError::IO(e) => {
                let msg = format!("Process exited unsuccessfully.\nReason: {e}");
                Err(msg.into())
            }
        },
    };

    let rendered = presentation::render_diags(
        &reporter,
        Some(&compiler_store.region_arena),
        &compiler_store.interner,
        &render_kind,
    );

    if reporter.diag_summary().has_err() {
        eprintln!("{rendered}");
    } else if reporter.diag_summary().has_warn() {
        println!("{rendered}");
    };

    if compiler_store.cfg.perf_tracker().can_use() {
        let perf_report = compiler_store.cfg.perf_tracker().form_report();
        for report_opt in &perf_report.time_reports {
            if let Some(report) = report_opt {
                dbg!(report);
                // let avg = report.time_spent / report.times as u32;
            }
        }
    }

    msg_res
}

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
    let chrn_cfg = ChrnConfig::default();

    let mut reporter = Reporter::new(crate::MAX_DIAGNOSTICS);
    let path = files::make_canon(&query_cmd.path)?;
    let src = match file_ops::fopen(&path) {
        Ok(f) => f,
        Err((_, err_msg)) => {
            let msg = format!("Process exited unsuccessfully.\nReason: {err_msg}");
            return Err(msg.into());
        }
    };

    //TODO: Fix this just make the msg dynamic

    // Please please please
    let (mut compiler, mut compiler_store, mut compiler_cache) =
        match constructors::create_compiler_with_cache(&path, src, &mut reporter, chrn_cfg) {
            Ok(data) => data,
            Err(init_err) => match init_err.cfg_err {
                ConfigLoadError::Diagnostic(diag) => {
                    let footers = presentation::make_footers(&reporter);
                    let render_cfg =
                        TerminalRenderConfig::new(glob_args.can_color, cli_cfg.terminal_color_type);

                    let rendered_diags = terminal_renderer::render_terminal_diags(
                        &[diag],
                        &footers,
                        // reporter.budget.amt_exceeded,
                        init_err.region.as_ref(),
                        &init_err.interner,
                        &render_cfg,
                    );

                    print_diags!(&rendered_diags);
                    let msg = "Failed to parse config file".to_string();
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
                        &reporter.diag_summary().diags(),
                        &footers,
                        Some(&compiler_store.region_arena),
                        &compiler_store.interner,
                        &render_cfg,
                    );

                    //TODO: Internally cut error message strings in the parser
                    print_diags!(&rendered_diags);
                    // Seems redundant to have this msg
                    // "Failed to parse config file".to_string().into()
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

//TODO: No. No terminal_renderer kind kinds
/// Runs embed cmd :=
fn exec_embed(
    embed_cmd: &EmbedCmd,
    glob_args: &GlobalArgs,
    cli_cfg: &CliConfig,
) -> Result<String, Option<String>> {
    // Centralized cmd to config construction for all known cmds?
    let mut chrn_cfg = ChrnConfig::default();
    let mut reporter = Reporter::new(crate::MAX_DIAGNOSTICS);

    // NOTE: Terminal only for now
    let render_kind = RenderKind::Terminal(TerminalRenderConfig::new(
        glob_args.can_color,
        cli_cfg.terminal_color_type,
    ));

    let src_path = files::make_canon(&embed_cmd.src_path)?;
    let dest_path = files::make_canon(&embed_cmd.dest_path)?;

    let src = match file_ops::fopen(&src_path) {
        Ok(f) => f,
        Err((_, err_msg)) => {
            let msg = format!("Process exited unsuccessfully.\nReason: {err_msg}");
            return Err(msg.into());
        }
    };

    // Maybe allow the different ouputs for checking
    //
    // Not sure how to lower this pasting because either, there is one extremely specific
    // method/function that does everything in this block of code, or we paste.
    // Will try the helper maybe.

    let region: SourceRegion = if embed_cmd.check {
        let (mut compiler, mut compiler_store, mut compiler_cache) =
            match constructors::create_compiler_with_cache(&src_path, src, &mut reporter, chrn_cfg)
            {
                Ok(data) => data,
                Err(init_err) => {
                    let msg_opt = match init_err.cfg_err {
                        ConfigLoadError::Diagnostic(diag) => {
                            reporter.push_safe(diag);
                            let msg = "`--check` failed, did not embed file".to_string();
                            msg.into()
                        }
                        ConfigLoadError::IO(err) => {
                            let msg = format!("Process exited unsuccessfully. Reason: {err}");
                            msg.into()
                        }
                    };

                    let rendered = presentation::render_diags(
                        &reporter,
                        init_err.region.as_ref(),
                        &init_err.interner,
                        &render_kind,
                    );
                    eprintln!("{rendered}");
                    return Err(msg_opt);
                }
            };

        let msg_res = match orchestrator::run_all(
            &mut reporter,
            &mut compiler,
            &mut compiler_store,
            Some(&mut compiler_cache),
        ) {
            // Is this redundant? Kinda just want it to say "complete" without "complete:"
            Ok(_) => Ok(()),
            Err(script_err) => match script_err {
                ScriptError::Parser | ScriptError::Semantic => Err(None),
                ScriptError::IO(e) => {
                    let msg = format!("Process exited unsuccessfully.\nReason: {e}");
                    Err(msg.into())
                }
            },
        };

        let rendered = presentation::render_diags(
            &reporter,
            Some(&compiler_store.region_arena),
            &compiler_store.interner,
            &render_kind,
        );

        if let Err(err_msg) = msg_res {
            eprintln!("{rendered}");
            return Err(err_msg);
        } else if reporter.diag_summary().has_warn() {
            println!("{rendered}");
        }

        // This is fine since main should be 0, unless internals broke. At that point the compiler
        // would need to directly take note of what the entry point was then we'd just use the id here.
        compiler_store
            .region_arena
            .swap_remove(SourceRegionId::new(0))
    } else {
        match module::extract_main(&src_path, src, &mut chrn_cfg) {
            Ok((main_mod, graph, interner, mut summary)) => {
                reporter.append_safe(&mut summary.diags);

                let rendered = presentation::render_diags(
                    &reporter,
                    Some(&graph.region_arena),
                    &interner,
                    &render_kind,
                );

                // If the region is broken then it's probably not the best idea to embed it
                if main_mod.state == ModuleState::BrokenRegion {
                    eprintln!("{rendered}");
                    let msg = presentation::EMBED_FAILURE_MSG.to_string();
                    return Err(msg.into());
                }

                if reporter.diag_summary().has_warn() {
                    println!("{rendered}");
                }

                // Taking out region from main since that's all we're interested in
                let mut arena = graph.region_arena;
                let main_region_id = main_mod.region_id.expect("Just created");
                arena.swap_remove(main_region_id)
            }
            Err(init_err) => {
                let msg_opt = match init_err.cfg_err {
                    ConfigLoadError::Diagnostic(diag) => {
                        reporter.push_safe(diag);
                        let msg = "`--check` failed, did not embed file".to_string();
                        msg.into()
                    }
                    ConfigLoadError::IO(err) => {
                        let msg = format!("Process exited unsuccessfully. Reason: {err}");
                        msg.into()
                    }
                };

                let rendered = presentation::render_diags(
                    &reporter,
                    init_err.region.as_ref(),
                    &init_err.interner,
                    &render_kind,
                );
                eprintln!("{rendered}");
                return Err(msg_opt);
            }
        }
    };

    //If script start is above 0, that means there is an "@def -> @end", and if serial start is `Some`,
    // that means there exists at least an `@end`.
    //
    // Both of these mean that there doesn't need to be any insertion of an @def or @end since they
    // are already self-contained regions
    let mut embed_portion = if region.script_start > 0 || region.serial_start.is_some() {
        Cow::Borrowed(&region.src_bytes)
    } else {
        // Wraps the src in @def[bytes]@end
        let def_end_size = keywords::REGION_CLAUSE_SIZE * 2;
        let mut altered_bytes = Vec::with_capacity(region.src_bytes.len() + def_end_size);
        altered_bytes.extend_from_slice(keywords::DEF_CLAUSE_STR.as_bytes());
        altered_bytes.extend_from_slice(&region.src_bytes);
        altered_bytes.extend_from_slice(keywords::END_CLAUSE_STR.as_bytes());
        Cow::Owned(altered_bytes)
    };

    // Bytes would mutate here
    //
    // Maybe if `--check` was chosen, we actually store the store, then if `Some` store we don't have
    // to run the parser again.
    if embed_cmd.fmt {
        todo!();
    } else if embed_cmd.minify {
        todo!();
    }

    let res = if embed_cmd.in_memory {
        filesc::write_bytes_front(&dest_path, &embed_portion)
    } else {
        filesc::write_bytes_front_stream(&dest_path, &embed_portion, "chrnstream-".into())
    };

    match res {
        Ok(_) => {
            let msg = format!(
                "Embedded\nsrc: {}\n ↓\ndest: {}",
                src_path.display(),
                dest_path.display()
            );
            Ok(msg)
        }
        Err(err) => Err(err.to_string().into()),
    }
}

//NOTE: Should this be a separate CLI tool but as an extension?
//No.
//Ok :
fn exec_doc(
    doc_cmd: &DocCmd,
    glob_args: &GlobalArgs,
    cli_cfg: &CliConfig,
) -> Result<String, Option<String>> {
    let x = byte_formatter!(2);
    dbg!(x);
    todo!()
}
