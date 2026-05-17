use std::path::Path;

use chrn_utils::intern::Intern;
use common::{chrn_settings::ChrnSettings, core_error::ConfigLoadError};
use script_lib::modules;

use crate::{script_prettifier::ScriptPrettifier, text_builder::TextBuilder};

//TODO: Trivia unit tests

//TEST:
pub fn fmt_script_block(path: &Path, settings: &ChrnSettings) -> Result<String, ConfigLoadError> {
    let mut interner = Intern::init();
    // Maybe a way to only load main?
    let script_compiler = modules::extract_modules(path, settings, &mut interner)?;

    // TEMP
    let module = &script_compiler.mods[0];

    let metadata = &module
        .src_metadata
        .as_ref()
        .expect("fmt can only work on valid paths");

    let (toks, trivia) = script_lib::lexer::Lexer::new(&metadata.src_bytes, metadata.script_start)
        .tokenize(&mut interner);

    let ast_info = match script_lib::parser::parse(settings, metadata, &toks, &mut interner) {
        Ok(info) => info,
        Err((unfinished_ast, _)) => unfinished_ast,
    };

    let src_str = match str::from_utf8(&metadata.src_bytes) {
        Ok(s) => s,
        Err(_) => todo!(),
    };

    let all_text_hir = TextBuilder::new(src_str, &ast_info, &toks, &interner, &trivia).form_hir();
    dbg!(&all_text_hir);
    panic!("hirring");

    let fmtted_script = ScriptPrettifier::new(src_str, &all_text_hir).fmt_script();
    dbg!(fmtted_script);

    todo!("fmt not done");
}
