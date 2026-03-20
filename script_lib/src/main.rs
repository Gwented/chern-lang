use std::{path::PathBuf, time::Instant};

use common::{intern::Intern, metadata::FileMetadata, storage::ConfigLoader};
use script_lib::{
    lexer::Lexer,
    linter,
    parser::{self},
    semantic::{
        constraint_resolver::ConstraintResolver, representation::Table, type_resolver::TypeResolver,
    },
};

//FIXME: More general file information that is persistent throughout the program which would
//include the file name, path, etc.

fn main() {
    let start = Instant::now();

    let path = PathBuf::from("./chrn_tests/main.chrn");

    let file = std::fs::File::open(&path).unwrap();

    let metadata = match ConfigLoader::new(&path, file).load_config() {
        Ok(meta) => meta,
        Err(e) => {
            // Why are all the errors for languages lowercase? Is there something I'm missing?
            // GREP? Wait it might actually be grep.
            eprintln!("From path => {}\n", path.display());
            eprintln!("error: {e}\nexiting...");
            std::process::exit(1);
        }
    };

    let mut interner = Intern::init();

    let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

    // Table table = new Table.tableFactoryFactoryGen();
    let mut table = Table::new();

    let ast_info = parser::parse(&metadata, &toks, &mut interner);

    linter::print_all(&ast_info, &interner);

    TypeResolver::new(&ast_info, &metadata, &interner, &mut table).resolve();
    ConstraintResolver::new(&ast_info, &metadata, &interner, &mut table).resolve();

    println!("{} ms", start.elapsed().as_millis());
}
