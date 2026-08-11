use super::helpers::*;
use crate::lookup::scopes::{
    AssociatedScopeKind, ScopeLookupPattern, ScopeLookupPreferenceFlags, find_sym_id,
};
use crate::semantic::hir::hir_symbols::SymbolKind;
use chrn_utils::id_types::ScopeId;

/// Runs lexing, parsing, and namespace resolution on a single-module script, returning the
/// compiler and interner so scope lookups can be performed directly.
fn ns_resolve(text: &str) -> (ScriptCompiler, Intern) {
    let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);

    let (mod_id, region) = {
        let module = &compiler.mods[ModuleId::new(0)];
        (module.mod_id, get_module_region(&arena, module))
    };

    let toks = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
        .tokenize(&mut interner)
        .toks;

    let ast_info = parser::parse(&settings, region, &toks, &interner).0;

    let reg_env = RegistrationEnv::new(&ast_info, region, mod_id);
    let (_, diags) = NamespaceResolver::new(&settings, &interner, &mut compiler).resolve(&reg_env);
    assert!(diags.diags.is_empty(), "{diags:?}");

    (compiler, interner)
}

/// Gets the symbol registered under `name` in the given scope of the module.
fn sym_in_scope(
    compiler: &ScriptCompiler,
    interner: &Intern,
    scope_type: ScopeType,
    name: &str,
) -> (ScopeId, SymbolId) {
    let name_id = interner
        .try_search_str(name)
        .unwrap_or_else(|| panic!("'{name}' was not interned"));
    let scope_id = compiler
        .get_scope_id(scope_type, ModuleId::new(0))
        .unwrap_or_else(|| panic!("{scope_type} scope should exist"));
    let sym_id = compiler.get_scope(scope_id).scope.table.interned_to_sym[&name_id];

    (scope_id, sym_id)
}

#[test]
fn scope_simple_test() {
    // -- NEUTRAL --
    let text = "
            let CONSTANT = 3
            ";

    let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);

    let (mod_id, region) = {
        let module = &compiler.mods[ModuleId::new(0)];
        (module.mod_id, get_module_region(&arena, module))
    };

    let toks = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
        .tokenize(&mut interner)
        .toks;

    let ast_info = parser::parse(&settings, region, &toks, &interner).0;

    let reg_env = RegistrationEnv::new(&ast_info, region, mod_id);
    let (_, _) = NamespaceResolver::new(&settings, &interner, &mut compiler).resolve(&reg_env);

    let module = &compiler.mods[ModuleId::new(0)];

    assert_eq!(module.scopes.len(), 3);
    assert_eq!(
        compiler.get_scope(module.scopes[1]).scope.scope_type,
        ScopeType::Compiler
    );

    // -- VAR --
    let text = "
            var->
                variable: i32
            ";

    let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);

    let (mod_id, region) = {
        let module = &compiler.mods[ModuleId::new(0)];
        (module.mod_id, get_module_region(&arena, module))
    };

    let toks = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
        .tokenize(&mut interner)
        .toks;

    let ast_info = parser::parse(&settings, region, &toks, &interner).0;

    let reg_env = RegistrationEnv::new(&ast_info, region, mod_id);
    let (_, _) = NamespaceResolver::new(&settings, &interner, &mut compiler).resolve(&reg_env);

    let module = &compiler.mods[ModuleId::new(0)];

    assert_eq!(module.scopes.len(), 3);
    assert_eq!(
        compiler.get_scope(module.scopes[2]).scope.scope_type,
        ScopeType::Var
    );

    // -- NEST --
    let text = "
            nest->
                struct Thing1 {}
                struct Thing2 {}
            ";

    let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);

    let (mod_id, region) = {
        let module = &compiler.mods[ModuleId::new(0)];
        (module.mod_id, get_module_region(&arena, module))
    };

    let toks = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
        .tokenize(&mut interner)
        .toks;

    let ast_info = parser::parse(&settings, region, &toks, &interner).0;

    let reg_env = RegistrationEnv::new(&ast_info, region, mod_id);
    let (_, _) = NamespaceResolver::new(&settings, &interner, &mut compiler).resolve(&reg_env);

    let module = &compiler.mods[ModuleId::new(0)];

    assert_eq!(module.scopes.len(), 3);
    assert_eq!(
        compiler.get_scope(module.scopes[2]).scope.scope_type,
        ScopeType::Nest
    );

    // -- All scopes --

    //TODO: Complex and Override
    let text = "
            let NEUTRAL = 3
            var->
                e#var: Nest
            nest->
                struct Nest {}
            ";

    let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);

    let (mod_id, region) = {
        let module = &compiler.mods[ModuleId::new(0)];
        (module.mod_id, get_module_region(&arena, module))
    };

    let toks = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
        .tokenize(&mut interner)
        .toks;

    let ast_info = parser::parse(&settings, region, &toks, &interner).0;

    let reg_env = RegistrationEnv::new(&ast_info, region, mod_id);
    let (_, _) = NamespaceResolver::new(&settings, &interner, &mut compiler).resolve(&reg_env);

    //TODO: Override and Complex
    let module = &compiler.mods[ModuleId::new(0)];
    assert_eq!(module.scopes.len(), 5);
    assert_eq!(
        compiler.get_scope(module.scopes[1]).scope.scope_type,
        ScopeType::Compiler
    );
    assert_eq!(
        compiler.get_scope(module.scopes[2]).scope.scope_type,
        ScopeType::Neutral
    );
    assert_eq!(
        compiler.get_scope(module.scopes[3]).scope.scope_type,
        ScopeType::Var
    );
}

#[test]
fn scope_lookup_pref_found_test() {
    // From `var` the scope order is Nest -> Neutral -> Compiler -> Core, so the struct is
    // seen before the variable.
    let text = "
            let Thing = 3
            nest->
                struct Thing {}
            ";

    let (compiler, interner) = ns_resolve(text);
    let thing_id = interner.try_search_str("Thing").unwrap();

    // A variable preference skips the struct found in `nest` and returns the variable
    // found later in `neutral`.
    let found = find_sym_id(
        &compiler,
        AssociatedScopeKind::Module(ModuleId::new(0)),
        thing_id,
        ScopeType::Var,
        ScopeLookupPattern::NoRestrictions,
        ScopeLookupPreferenceFlags::new(ScopeLookupPreferenceFlags::VARIABLE.into()),
    )
    .expect("Symbol should be found");

    let (neutral_scope_id, var_sym_id) =
        sym_in_scope(&compiler, &interner, ScopeType::Neutral, "Thing");
    assert_eq!(found.found_sym_id, var_sym_id);
    assert_eq!(found.scope_found_in, neutral_scope_id);
    assert!(matches!(
        compiler.symbols[found.found_sym_id].kind,
        SymbolKind::Variable(_)
    ));

    // A type preference matches the struct on the first scope searched.
    let found = find_sym_id(
        &compiler,
        AssociatedScopeKind::Module(ModuleId::new(0)),
        thing_id,
        ScopeType::Var,
        ScopeLookupPattern::NoRestrictions,
        ScopeLookupPreferenceFlags::new(ScopeLookupPreferenceFlags::TYPE.into()),
    )
    .expect("Symbol should be found");

    let (nest_scope_id, struct_sym_id) =
        sym_in_scope(&compiler, &interner, ScopeType::Nest, "Thing");
    assert_eq!(found.found_sym_id, struct_sym_id);
    assert_eq!(found.scope_found_in, nest_scope_id);
    assert!(matches!(
        compiler.symbols[found.found_sym_id].kind,
        SymbolKind::Type(_)
    ));
}

#[test]
fn scope_lookup_pref_fallback_test() {
    // Only a type named `Thing` exists, so a variable preference can't be satisfied and the
    // lookup falls back to the non-preferred symbol it did find.
    let text = "
            nest->
                struct Thing {}
            ";

    let (compiler, interner) = ns_resolve(text);
    let thing_id = interner.try_search_str("Thing").unwrap();

    let found = find_sym_id(
        &compiler,
        AssociatedScopeKind::Module(ModuleId::new(0)),
        thing_id,
        ScopeType::Nest,
        ScopeLookupPattern::NoRestrictions,
        ScopeLookupPreferenceFlags::new(ScopeLookupPreferenceFlags::VARIABLE.into()),
    )
    .expect("Fallback should return the non-preferred symbol");

    let (nest_scope_id, struct_sym_id) =
        sym_in_scope(&compiler, &interner, ScopeType::Nest, "Thing");
    assert_eq!(found.found_sym_id, struct_sym_id);
    assert_eq!(found.scope_found_in, nest_scope_id);

    // Mirror: only a variable named `Thing` exists, so a type preference falls back to it.
    let text = "
            let Thing = 3
            ";

    let (compiler, interner) = ns_resolve(text);
    let thing_id = interner.try_search_str("Thing").unwrap();

    let found = find_sym_id(
        &compiler,
        AssociatedScopeKind::Module(ModuleId::new(0)),
        thing_id,
        ScopeType::Neutral,
        ScopeLookupPattern::NoRestrictions,
        ScopeLookupPreferenceFlags::new(ScopeLookupPreferenceFlags::TYPE.into()),
    )
    .expect("Fallback should return the non-preferred symbol");

    let (neutral_scope_id, var_sym_id) =
        sym_in_scope(&compiler, &interner, ScopeType::Neutral, "Thing");
    assert_eq!(found.found_sym_id, var_sym_id);
    assert_eq!(found.scope_found_in, neutral_scope_id);
}

#[test]
fn scope_lookup_no_preference_test() {
    // From `var` the scope order is Nest -> Neutral -> Compiler -> Core, so with no
    // preference the struct is returned since `nest` is searched first.
    let text = "
            let Thing = 3
            nest->
                struct Thing {}
            ";

    let (compiler, interner) = ns_resolve(text);
    let thing_id = interner.try_search_str("Thing").unwrap();

    let found = find_sym_id(
        &compiler,
        AssociatedScopeKind::Module(ModuleId::new(0)),
        thing_id,
        ScopeType::Var,
        ScopeLookupPattern::NoRestrictions,
        ScopeLookupPreferenceFlags::none(),
    )
    .expect("Symbol should be found");

    let (_, struct_sym_id) = sym_in_scope(&compiler, &interner, ScopeType::Nest, "Thing");
    assert_eq!(found.found_sym_id, struct_sym_id);
}
