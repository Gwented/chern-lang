use std::time::Instant;

use common::{intern::Intern, storage::FileLoader};
use script_lib::{
    lexer::Lexer,
    linter,
    parser::{self},
};

fn main() {
    let start = Instant::now();

    let path = "./chrn_tests/main.chrn";

    let file = std::fs::File::open(path).unwrap();

    let (data, lex_start, _) = match FileLoader::new(file).load_config() {
        Ok((data, lex_start, serial_start)) => (data, lex_start, serial_start),
        Err(e) => {
            eprintln!("Error: {e}\nAborting...");
            std::process::exit(1);
        }
    };

    let mut interner = Intern::init();

    let toks = Lexer::new(&data, lex_start).tokenize(&mut interner);

    let ast = parser::parse(&data, &toks, &mut interner);

    //TODO: Make linter lint not print
    linter::print_all(&ast, &interner);

    println!("{} ms", start.elapsed().as_millis());
}
