use chrn_utils::intern::Intern;
use compilation::{
    lookup::scopes,
    script_compiler::ScriptCompiler,
    semantic::hir::hir_concepts::{MemberSymbolKind, Symbol},
};

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
    target_ident: &str,
) -> (Vec<&'a Symbol>, Vec<&'a MemberSymbolKind>) {
    let target_name_id = match interner.try_search_str(target_ident) {
        Some(id) => id,
        None => return (Vec::new(), Vec::new()),
    };

    scopes::find_symbols_named_ref(&compiler, target_name_id)
}

pub fn find_symbols_named_from_module<'a>(
    compiler: &'a ScriptCompiler,
    interner: &Intern,
    ident: &str,
    target_mod_ident: &str,
    // For allowing duplicate named modules to be searched if there are multiple under the same
    // identifier
    // seen_mods: Vec<PathId>,
) -> (Vec<&'a Symbol>, Vec<&'a MemberSymbolKind>) {
    // Should probably just, not take all by default (eventually)
    // interner.try_search_str(idx)
    // scopes::find_symbols_named_from_module_ref(compiler, interner, 0, ident)
    todo!()
}
