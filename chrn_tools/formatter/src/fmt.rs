use std::path::Path;

use chrn_utils::{chrn_settings::ChrnSettings, core_error::ConfigLoadError, intern::Intern};
use script_lib::modules;

use crate::{script_prettifier::ScriptPrettifier, text_builder::TextBuilder};

//TODO: Trivia unit tests

//TEST:
pub fn fmt_script_block(path: &Path, settings: &ChrnSettings) -> Result<String, ConfigLoadError> {
    let mut interner = Intern::init();
    // Maybe a way to only load main?
    let (script_compiler, region_arena) = modules::extract_modules(path, settings, &mut interner)?;

    // TEMP
    let module = &script_compiler.mods[0];

    let region_id = module
        .region_id
        .as_ref()
        .expect("fmt can only work on valid paths");

    let metadata = region_arena.extract_region(*region_id);

    let (toks, trivia) = script_lib::lexer::Lexer::new(
        metadata.region_id,
        &metadata.src_bytes,
        metadata.script_start,
    )
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
