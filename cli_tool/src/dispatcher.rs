use std::io;

use interpreter_lib::interpreter;

use crate::args::{CheckCmd, Cli, Commands};

// Anyhow?
pub fn exec(cli: Cli) -> Result<(), ()> {
    match cli.command {
        Commands::Check(check_cmd) => process_check(check_cmd),
        Commands::FMT(fmt_cmd) => todo!(),
        Commands::Gen(gen_cmd) => todo!(),
    }
}

// Would call library with interpreter
// Does this need a result?
// What if this had a probability model?
fn process_check(check_cmd: CheckCmd) -> Result<(), ()> {
    // Will clean output
    match interpreter::interpret_chrn_cfg(&check_cmd.path) {
        Ok(_) => {
            println!("No errors found in {}", check_cmd.path.display());
        }
        Err(e) => match e.kind() {
            io::ErrorKind::NotFound => {
                panic!();
            }
            io::ErrorKind::PermissionDenied => todo!(),
            io::ErrorKind::IsADirectory => todo!(),
            io::ErrorKind::TooManyLinks => todo!(),
            io::ErrorKind::InvalidFilename => todo!(),
            io::ErrorKind::Interrupted => todo!(),
            _ => todo!(),
        },
    };

    Ok(())
}
