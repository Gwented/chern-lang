// THIS WAS PASTED FROM AN OLD PROJECT MANY OF THIS IS LIKELY OFF
//TODO: ADD RELEVANT COMMANDS

use std::{env, path::PathBuf};

use clap::{Parser, error::RichFormatter};
use clap_derive::{Args, Parser, Subcommand};

// Not sure where to put this...
/// Wrapper functions over `Cli::try_parse()` which tries to account for possible tooling that may
/// using naming like "chrn-*" before returning an error
pub fn try_parse() -> Result<Cli, clap::error::Error<RichFormatter>> {
    let args: Vec<String> = env::args().collect();

    // If the binary was executed as chrn-<cmd> (e.g. chrn-fmt), it attempts to extract the
    // subcommand from the binary name and re-parse.
    if let Some(bin_name) = args.first().and_then(|s| s.strip_prefix("chrn-")) {
        let mut new_args = vec!["chrn".to_string(), bin_name.to_string()];
        new_args.extend_from_slice(&args[1..]);
        return Cli::try_parse_from(new_args);
    }

    match Cli::try_parse() {
        Ok(cli) => Ok(cli),
        Err(err) => {
            // Parsing failed so checking whether args[1] matches an external binary
            // named chrn-<subcommand> and, if so, delegates to it.
            if let Some(candidate) = args.get(1) {
                //FIX: MAKE SURE WINDOWS IS ALIVE
                if let Some(path) = find_external_binary(candidate) {
                    let status = std::process::Command::new(&path)
                        .args(&args[2..])
                        .status()
                        .unwrap_or_else(|_| std::process::exit(1));
                    std::process::exit(status.code().unwrap_or(1));
                }
            }
            // This saddens me greatly.
            Err(err)
        }
    }
}

// CHECK WINDOWS
/// Looks up `chrn-{subcommand}` in every directory listed in PATH variable depending on OS
fn find_external_binary(subcommand: &str) -> Option<PathBuf> {
    let bin_name = format!("chrn-{subcommand}");
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths).find_map(|dir| {
            let full = dir.join(&bin_name);
            if full.is_file() {
                return Some(full);
            }
            #[cfg(windows)]
            {
                let full_exe = dir.join(format!("{bin_name}.exe"));
                if full_exe.is_file() {
                    return Some(full_exe);
                }
            }
            None
        })
    })
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
