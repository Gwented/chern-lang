//TODO: Eventually will have it's own cli backend but not priority
// Maybe not?
use chrn::{args, config::CliConfig, dispatcher};
use clap::Parser;
use common::color;

fn main() {
    let cli_cfg = CliConfig::new();
    let cli = args::Cli::parse();

    // Why was this noted?
    // Is borrowed so that the program success or error messages can be colored from one place.
    match dispatcher::exec(&cli, &cli_cfg) {
        Ok(msg) => {
            let (green, nc) = color::get_green(cli_cfg.can_color, cli_cfg.terminal_color_type);
            println!("{green}complete{nc}: {msg}");
        }
        Err(emsg) => {
            let (red, nc) = color::get_red(cli_cfg.can_color, cli_cfg.terminal_color_type);
            println!("{red}exited{nc}: {emsg}");
            std::process::exit(1);
        }
    }
}
