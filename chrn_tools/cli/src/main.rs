//TODO: Eventually will have it's own cli backend but not priority
use chrn::{args, config::CliConfig, dispatcher};
use clap::Parser;
use common::color;

fn main() {
    let cli_cfg = CliConfig::new();
    let cli = args::Cli::parse();

    // Is borrowed so that the program success or error messages can be colored from one place.
    match dispatcher::exec(&cli, &cli_cfg) {
        Ok(msg) => {
            let (green, nc) = color::get_green(cli_cfg.can_color);
            println!("{green}complete{nc}: {msg}");
        }
        Err(emsg) => {
            let (red, nc) = color::get_red(cli_cfg.can_color);
            println!("{red}exited{nc}: {emsg}");
        }
    }
}
