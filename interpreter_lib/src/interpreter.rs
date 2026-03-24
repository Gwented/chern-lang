use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use common::{
    config_loader::FileLoader,
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
pub fn interpret_chrn_cfg<R: Read>(src: R, path: &Path) -> Result<(), CoreError> {
    let metadata = FileLoader::new(path, src).load_config()?;

    let mut interner = Intern::init();

    let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

    let mut table = Table::new();

    let ast_info = parser::parse(&metadata, &toks, &mut interner);

    TypeResolver::new(&ast_info, &metadata, &interner, &mut table).resolve();
    ConstraintResolver::new(&ast_info, &metadata, &interner, &mut table).resolve();

    // Need to cache stuffies

    // IR will be very different
    Ok(())
}
