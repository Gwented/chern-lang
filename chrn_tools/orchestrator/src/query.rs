use chrn_utils::intern::Intern;
use script_lib::{
    script_compiler::ScriptCompiler,
    semantic::{representation::Symbol, scopes},
};

use crate::chrn_manager::ChrnManager;

// Testing something..

pub struct CompilerQueryConfig {}

impl CompilerQueryConfig {
    pub fn new() -> CompilerQueryConfig {
        CompilerQueryConfig {}
    }
}

pub fn find_symbols_named<'a>(
    compiler: &'a ScriptCompiler,
    interner: &Intern,
    ident: &str,
) -> Vec<&'a Symbol> {
    scopes::find_symbols_named(&compiler, interner, ident)
}

pub fn find_module_symbols_named<'a>(
    chrn_manager: &'a ChrnManager,
    ident: &str,
) -> Vec<&'a Symbol> {
    // Should probably just, not take all by default (eventually)
    let found_syms =
        scopes::find_symbols_named(&chrn_manager.compiler, &chrn_manager.interner, ident);
    todo!()
}

