// THIS WAS PASTED FROM AN OLD PROJECT MANY OF THIS IS LIKELY OFF
//TODO: ADD RELEVANT COMMANDS

use std::path::PathBuf;

use clap_derive::{Args, Parser, Subcommand};

//TODO: ADD ABOUT
#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub(crate) command: Commands,
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
