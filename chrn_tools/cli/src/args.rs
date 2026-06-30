//TODO: ADD RELEVANT COMMANDS

use std::env;
use std::path::PathBuf;

use clap::{Parser, error::RichFormatter};
use clap_derive::{Args, Parser, Subcommand};

use crate::detect;

/// Wraps `Cli::try_parse()` and layers two extension hooks on top of it:
///
/// 1. If the binary itself is named `chrn-<subcommand>`, treat the trailing
///    portion as the subcommand and re-parse as if the user had typed
///    `chrn <subcommand> ...`.
/// 2. If clap rejects the input because of an unknown subcommand, fall back
///    to looking up `chrn-<subcommand>` on `PATH` and hand control to that
///    external binary if it exists.
pub fn try_parse() -> Result<Cli, clap::error::Error<RichFormatter>> {
    let args: Vec<String> = env::args().collect();

    if let Some(sub) = detect::subcommand_from_bin_name(&args) {
        let mut new_args = Vec::with_capacity(args.len() + 1);
        new_args.push("chrn".to_string());
        new_args.push(sub.to_string());
        new_args.extend_from_slice(&args[1..]);
        return Cli::try_parse_from(new_args);
    }

    match Cli::try_parse() {
        Ok(cli) => Ok(cli),
        Err(err) => {
            if let Some(candidate) = args.get(1)
                && let Some(path) = detect::find_external_binary(candidate)
            {
                delegate_to_external(&path, &args[2..]);
            }
            Err(err)
        }
    }
}

/// Spawns `path` with the supplied `forwarded_args`, mirroring the spawned
/// process's exit status into the current process. If the spawn itself
/// fails, the process exits with code `1`. This function does not return.
fn delegate_to_external(path: &std::path::Path, forwarded_args: &[String]) -> ! {
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
}

#[derive(Args)]
pub struct CheckCmd {
    /// Path of `.chrn` config file to check
    pub(crate) path: PathBuf,
    #[arg(short = 'l', long = "lint", default_value_t = false)]
    pub(crate) can_lint: bool,
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
