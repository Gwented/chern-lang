use std::path::Path;

use chrn_utils::intern::Intern;
use common::{chrn_settings::ChrnSettings, core_error::ConfigLoadError};
use script_lib::modules;

//TODO: Will be continued after a Token is added to track whitespace and comments within the lexer
pub fn fmt_script_block(path: &Path, settings: &ChrnSettings) -> Result<String, ConfigLoadError> {
    let mut interner = Intern::init();
    let script_compiler = modules::extract_modules(path, settings, &mut interner)?;

    for mod_idx in 0..script_compiler.mods.len() {
        let module = &script_compiler.mods[mod_idx];
        // TEMP
        if mod_idx == script_compiler.std_mod_id.id {
            continue;
        }

        let metadata = module
            .metadata
            .as_ref()
            .expect("std should not be resolved");

        let toks = script_lib::lexer::Lexer::new(&metadata.src_bytes, metadata.script_start)
            .tokenize(&mut interner);

        dbg!(toks);
        panic!();
        // let ast_info = match script_lib::parser::parse(settings, &module, &toks, &mut interner) {
        //     Ok(info) => info,
        //     Err((unfinished_ast, _)) => unfinished_ast,
        // };
    }

    todo!()
}
