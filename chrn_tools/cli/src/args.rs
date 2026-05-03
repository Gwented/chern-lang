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
    /// Formats `.chrn` file prettily
    Fmt(FmtCmd),
    Gen(GenCmd),
    #[command(name = "details", alias = "d")]
    Details(DetailsCmd),
}

#[derive(Args)]
pub struct CheckCmd {
    /// Path of `.chrn` config file to check
    pub(crate) path: PathBuf,
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
    #[arg(short = 'm', long = "minify", default_value_t = false)]
    pub(crate) minify: bool,
}

#[derive(Args)]
pub struct DetailsCmd {
    /// Path of `.chrn` file to display details of
    pub(crate) path: PathBuf,
}

//     #[arg(short = 'l', long = "log", default_value_t = false)]
