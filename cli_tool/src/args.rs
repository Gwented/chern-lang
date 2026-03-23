// THIS WAS PASTED FROM AN OLD PROJECT MANY OF THIS IS LIKELY OFF

use std::path::PathBuf;

use clap_derive::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Runs interpreter over given `.chrn` file to check for syntax errors
    #[command(name = "check")]
    Check(CheckCmd),
}

#[derive(Args)]
pub struct CheckCmd {
    /// Checks given chern properties file for syntax errors
    // #[arg(short = 'l', long = "log")]
    pub path: PathBuf,
}

// #[derive(Args)]
// pub struct ScanCmd {
//     /// Generate logs within '.wtree/logs/' of any errors that occured.
//     #[arg(short = 'l', long = "log", default_value_t = false)]
//     pub can_log: bool,
// }
