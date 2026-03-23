use chrn::{args, dispatcher};
use clap::Parser;

fn main() {
    // Need metadata file for cli config
    let cli = args::Cli::parse();

    if let Err(e) = dispatcher::exec(cli) {
        todo!("error(main)");
    };
}
