use interpreter_lib::interpreter;

use crate::args::{CheckCmd, Cli, Commands};

pub fn exec(cli: Cli) -> Result<(), ()> {
    match cli.command {
        Commands::Check(check_cmd) => process_check(check_cmd),
    }
}

// Would call library with interpreter
// Does this need a result?
fn process_check(check_cmd: CheckCmd) -> Result<(), ()> {
    // Will clean output
    match interpreter::interpret_chrn_cfg(check_cmd.path) {
        Ok(_) => (),
        Err(_) => todo!(),
    };

    Ok(())
}
