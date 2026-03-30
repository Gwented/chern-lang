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
    Fmt(FMTCmd),
    Gen(GenCmd),
}

#[derive(Args)]
pub struct CheckCmd {
    /// Path of `.chrn` config file to check
    pub(crate) path: PathBuf,
}

// chrn gen <PATH> --<LANGUAGE> <TypeName>

#[derive(Args)]
pub struct GenCmd {
    /// Generates `chrn` config file
    pub(crate) path: PathBuf,
}

#[derive(Args)]
pub struct FMTCmd {
    /// Path of `.chrn` config file to format
    pub(crate) path: PathBuf,
}

//     #[arg(short = 'l', long = "log", default_value_t = false)]
