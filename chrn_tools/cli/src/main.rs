//TODO: Eventually will have it's own cli backend but not priority
// Maybe not?
use chrn::{args, config::CliConfig, dispatcher};
use clap::Parser;
use common::color;

fn main() {
    let cli_cfg = CliConfig::new();
    let cli = args::Cli::parse();

    match dispatcher::exec(&cli, &cli_cfg) {
        Ok(msg) => {
            let (green, nc) =
                color::get_green(cli.glob_args.can_color, cli_cfg.terminal_color_type);
            println!("{green}complete{nc}: {msg}");
        }
        Err(err_msg) => {
            let (red, nc) = color::get_red(cli.glob_args.can_color, cli_cfg.terminal_color_type);
            println!("{red}exited{nc}: {err_msg}");
            std::process::exit(1);
        }
    }
}
