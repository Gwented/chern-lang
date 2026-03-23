use std::{
    fs,
    path::{Path, PathBuf},
};

use common::{config_loader::ConfigLoader, intern::Intern};
use script_lib::{
    lexer::Lexer,
    parser,
    semantic::{
        constraint_resolver::ConstraintResolver, representation::Table, type_resolver::TypeResolver,
    },
};

//TEST:
//Could make it so all the context parts are public, allowing them to be returned and emit errors
// from a matching of a result for whoever is using the resolver, lexer, etc.
pub fn interpret_chrn_cfg(path: &Path) -> Result<(), std::io::Error> {
    let file = fs::File::open(&path)?;

    let metadata = match ConfigLoader::new(&path, file).load_config() {
        Ok(meta) => meta,
        Err(e) => {
            eprintln!("From path => \"{}\"\n", path.display());
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

    // Table should be made into a strictly curated piece of data for serial to look at

    // IR might be very different
    Ok(())
}
