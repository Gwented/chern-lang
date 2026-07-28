use std::env;
use std::path::PathBuf;

use clap::{Parser, error::RichFormatter};
use clap_derive::{Args, Parser, Subcommand};

use crate::{config::CliConfig, detect};

/// Parses argv with clap first; only if clap rejects the input do the
/// two extension hooks get a chance to recover:
///
/// 1. If the binary itself was invoked as `chrn-<subcommand>`, the
///    trailing portion is treated as the subcommand and clap is re-run
///    as if the user had typed `chrn <subcommand> ...`.
/// 2. Otherwise, look up `chrn-<subcommand>` on `PATH` and hand control
///    to that external binary if it exists.
pub fn try_parse(cli_cfg: &CliConfig) -> Result<Cli, clap::error::Error<RichFormatter>> {
    let err = match Cli::try_parse() {
        Ok(cli) => return Ok(cli),
        Err(err) => {
            //SAFETY
            // If user has not explicitly checked extensions as a feature it will not execute
            // arbitrary code
            if !cli_cfg.env_var_repo.chrn_extensions {
                return Err(err);
            }

            err
        }
    };

    // Cheap to collect once and reuse for both extension hooks.
    let args: Vec<String> = env::args().collect();

    // 1. `chrn-<sub>` binary alias: synthesize the missing subcommand
    //    and let clap try again on the rewritten argv.
    if let Some(first_arg) = args.first() {
        if let Some(sub) = detect::subcommand_from_bin_name(first_arg) {
            let mut rewritten = Vec::with_capacity(args.len() + 1);
            rewritten.push("chrn".to_string());
            rewritten.push(sub.to_string());
            rewritten.extend_from_slice(&args[1..]);
            return Cli::try_parse_from(rewritten);
        }
    }

    // 2. External `chrn-<sub>` on `PATH` for a subcommand clap doesn't
    //    know about. `find_external_binary` will simply miss for any
    //    candidate that isn't a real `chrn-<x>` on `PATH`, so unrelated
    //    clap errors (bad flag, missing value, etc.) fall through to
    //    the original error below.
    if let Some(candidate) = args.get(1)
        && let Some(path) = detect::find_external_binary(candidate)
    {
        delegate_to_external(&path, &args[2..]);
    }

    Err(err)
}

/// Spawns `path` with the supplied `forwarded_args`, mirroring the spawned
/// process's exit status into the current process. If the spawn itself
/// fails, the process exits with code `1`. This function does not return.
fn delegate_to_external(path: &std::path::Path, forwarded_args: &[String]) -> ! {
    // Kind of want to enforce that this requires a chrn env variable so that arbitrary code can't
    // be executed just because it manipulates itself to use the chrn suffix
    let status = std::process::Command::new(path)
        .args(forwarded_args)
        .status()
        .unwrap_or_else(|_| std::process::exit(1));
    std::process::exit(status.code().unwrap_or(1));
}

//TODO: ADD ABOUT
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(flatten)]
    pub glob_args: GlobalArgs,

    #[command(subcommand)]
    pub(crate) command: Commands,
}

#[derive(Args)]
pub struct GlobalArgs {
    /// Disables colored output
    #[arg(
        long = "no-color",
        action = clap::ArgAction::SetFalse,
        global = true,
        default_value_t = true
    )]
    pub can_color: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Runs interpreter over given `.chrn` file to check for syntax errors
    #[command(name = "check", alias = "c")]
    Check(CheckCmd),
    // Minify argument
    /// Formats `.chrn` file for readability. Changes behavior with arguments.
    Fmt(FmtCmd),
    Gen(GenCmd),
    #[command(name = "query", alias = "q")]
    Query(QueryCmd),
    // DON'T CAREe DIDNT ASK
    /// Embeds a region src into a destination. This inserts `@def` and `@end` by default
    /// after embedding the region if not already present in the src file.
    /// Embedding only fails on region errors such as `@def` without `@end` or exceeding max region
    /// size. Embedding uses constant memory by creating a file, embedding script into it,
    /// streaming data from the original file into it,removing the orignal file
    /// then renaming the new file to be the same as the original.
    #[command(name = "embed", alias = "e")]
    Embed(EmbedCmd),
}

#[derive(Args)]
pub struct CheckCmd {
    /// Path of `.chrn` config file to check
    pub(crate) path: PathBuf,
    /// Allows for lint warns to be applied, which aren't by default
    #[arg(short = 'l', long = "lint", default_value_t = false)]
    pub(crate) can_lint: bool,
    /// Emits developer debug info during check
    #[arg(long = "dbg", default_value_t = false)]
    pub(crate) dbg_mode: bool,
    /// Emits diagnostics as a JSON document in stderr
    #[arg(long = "json", default_value_t = false)]
    pub(crate) json: bool,
    /// Emits diagnostics as a YAML document in stderr
    /// When combined with `--json`, JSON is emitted.
    #[arg(long = "yaml", default_value_t = false)]
    pub(crate) yaml: bool,
    /// Minifies output iff JSON or YAML output is chosen
    #[arg(short = 'm', long = "minify", default_value_t = false)]
    pub(crate) minify: bool,
}

/// For `embed` cmd
#[derive(Args)]
pub struct EmbedCmd {
    /// Path of file to extract chrn region from
    pub(crate) src_path: PathBuf,
    /// Path of file to put the extracted chrn file into
    pub(crate) dest_path: PathBuf,
    /// Changes behavior from streaming to loading all of `dest` in to memory.
    #[arg(long = "in-memory", default_value_t = false)]
    pub(crate) in_memory: bool,
    //TODO: Maybe just separate the minify and fmt cmd, like overall.
    // Ok but what if you could point cli format calls to a different binary through an env var or
    // something of that particular individual exact kind of conceptual topic?
    /// Formats output with opinionated formatter
    #[arg(long = "fmt", default_value_t = false)]
    pub(crate) fmt: bool,
    /// Formats output to be minified
    #[arg(short = 'm', long = "minify", default_value_t = false)]
    pub(crate) minify: bool,
    /// Enforces that src is a valid chrn file before inserting.
    #[arg(short = 'c', long = "check", default_value_t = false)]
    pub(crate) check: bool,
}

// chrn gen <PATH> --<LANGUAGE> man<TypeName>

#[derive(Args)]
pub struct GenCmd {
    /// Generates `chrn` script file
    pub(crate) path: PathBuf,
    /// Name of what to data to generate
    pub(crate) type_name: String,
}

#[derive(Args)]
pub struct FmtCmd {
    /// Path of `.chrn` file to format
    pub(crate) path: PathBuf,
    /// Minifies file
    #[arg(short = 'm', long = "minify", default_value_t = false)]
    pub(crate) minify: bool,
}

// Shows all modules by default
#[derive(Args)]
pub struct QueryCmd {
    /// Path of `.chrn` file to display the structural details of
    pub(crate) path: PathBuf,
    /// Identifier to match the prefix of and query code for any instances of
    pub(crate) ident: Option<String>,
    /// The exact identifier should be matched instead of the prefix
    #[arg(long = "exact", requires("ident"))]
    pub(crate) exact: bool,
    /// Only displays the entry point's information
    #[arg(
        long = "entry-only",
        default_value_t = false,
        // conflicts_with = "only_modules",
        conflicts_with = "skip_modules",
    )]
    pub(crate) entry_only: bool,
    /// Only displays information for the modules listed
    #[arg(
        long = "only-modules",
        conflicts_with = "entry_only",
        conflicts_with = "skip_modules"
    )]
    pub(crate) only_modules: Vec<String>,
    /// Does not display information regarding any module under the identifiers given
    #[arg(
        long = "skip-modules",
        conflicts_with = "entry_only",
        conflicts_with = "only_modules"
    )]
    pub(crate) skip_modules: Vec<String>,
}

//     #[arg(short = 'l', long = "log", default_value_t = false)]
