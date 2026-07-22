//TODO: Eventually will have it's own cli backend but not priority
// Maybe not?
use chrn::{args, config::CliConfig, dispatcher};
use common::color;

fn main() {
    let cli_cfg = CliConfig::init();
    // Checks this first so external tooling syntax can be checked before exiting
    let cli = match args::try_parse(&cli_cfg) {
        Ok(c) => c,
        Err(err) => err.exit(),
    };

    match dispatcher::exec(&cli, &cli_cfg) {
        Ok(msg) => {
            let (green, nc) =
                color::get_green(cli.glob_args.can_color, cli_cfg.terminal_color_type);
            println!("{green}complete{nc}: {msg}");
        }
        Err(err_msg_opt) => {
            if let Some(err_msg) = err_msg_opt {
                let (red, nc) =
                    color::get_red(cli.glob_args.can_color, cli_cfg.terminal_color_type);
                println!("{red}exited{nc}: {err_msg}");
            }
            std::process::exit(1);
        }
    }
}
