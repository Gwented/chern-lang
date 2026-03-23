use std::path::PathBuf;

use common::{intern::Intern, storage::ConfigLoader};
use script_lib::{
    lexer::Lexer,
    parser,
    semantic::{
        constraint_resolver::ConstraintResolver, representation::Table, type_resolver::TypeResolver,
    },
};

//TEST:
pub fn interpret_chrn_cfg(path: PathBuf) -> Result<(), std::io::Error> {
    let file = std::fs::File::open(&path)?;

    let metadata = match ConfigLoader::new(&path, file).load_config() {
        Ok(meta) => meta,
        Err(e) => {
            eprintln!("From path => {}\n", path.display());
            eprintln!("error: {e}\nexiting...");
            std::process::exit(1);
        }
    };

    let mut interner = Intern::init();

    let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

    let mut table = Table::new();

    let ast_info = parser::parse(&metadata, &toks, &mut interner);

    TypeResolver::new(&ast_info, &metadata, &interner, &mut table).resolve();
    ConstraintResolver::new(&ast_info, &metadata, &interner, &mut table).resolve();

    Ok(())
}
