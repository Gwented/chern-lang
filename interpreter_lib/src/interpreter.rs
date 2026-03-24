use std::{
    fs,
    path::{Path, PathBuf},
};

use common::{
    config_loader::ConfigLoader,
    core_error::{ConfigLoadError, CoreError},
    intern::Intern,
};
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
// Likely returning Core error
pub fn interpret_chrn_cfg(path: &Path) -> Result<(), CoreError> {
    let file = match fs::File::open(&path) {
        Ok(f) => f,
        Err(e) => return Err(CoreError::Config(ConfigLoadError::IO(e))),
    };

    let metadata = ConfigLoader::new(&path, file).load_config()?;

    let mut interner = Intern::init();

    let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

    let mut table = Table::new();

    let ast_info = parser::parse(&metadata, &toks, &mut interner);

    TypeResolver::new(&ast_info, &metadata, &interner, &mut table).resolve();
    ConstraintResolver::new(&ast_info, &metadata, &interner, &mut table).resolve();

    // Table should be made into a strictly curated piece of data for serial to look at
    // Need to cache stuffies

    // IR might be very different
    Ok(())
}
