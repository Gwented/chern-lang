use super::helpers::*;
use crate::lookup::scopes::find_sym_id;
use crate::lookup::scopes::scopes_concepts::{
    AssociatedScopeKind, ScopeLookupPattern, ScopeLookupPreferenceFlags, ScopeType,
};
use crate::script_compiler::compiler_constants::builtin_ty_to_id;
use crate::script_compiler::helpers::core_helpers::CORE_BUILTIN_TYPES_DATASET;
use crate::semantic::hir::hir_concepts::Type;
use crate::semantic::hir::hir_symbols::SymbolKind;
use chrn_utils::id_types::{ScopeId, TypeId};
use chrn_utils::intern;

/// Runs lexing, parsing, and namespace resolution on a single-module script, returning the
/// compiler and interner so scope lookups can be performed directly.
fn ns_resolve(text: &str) -> (ScriptCompiler, Intern) {
    resolve_single_module(text, Stage::Namespace)
        .expect_ok()
        .into_state()
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

    let (compiler, _) = ns_resolve(text);

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

    let (compiler, _) = ns_resolve(text);

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

    let (compiler, _) = ns_resolve(text);

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

    let (compiler, _) = ns_resolve(text);

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

/// Every built-in that declares an intrinsic namespace, as `(source name, its `TypeId`)`.
fn namespaced_builtins() -> Vec<(String, TypeId)> {
    let interner = Intern::init();

    CORE_BUILTIN_TYPES_DATASET
        .iter()
        .filter(|(_, _, ns)| !ns.is_empty())
        .map(|(interned, builtin_ty, _)| {
            (
                interner.search_idx(*interned as usize).to_string(),
                TypeId::new(builtin_ty_to_id(builtin_ty.kind())),
            )
        })
        .collect()
}

/// The scope a built-in's symbol owns, which is where its intrinsic constants live.
fn ns_scope_of(compiler: &ScriptCompiler, type_id: TypeId) -> ScopeId {
    let Type::BuiltinTypeInfo(info) = &compiler.types[type_id].ty else {
        panic!("expected a builtin at {type_id:?}");
    };

    match compiler.symbols[info.sym_id].associated_scope {
        Some(AssociatedScopeKind::Scope(scope_id)) => scope_id,
        other => panic!("builtin at {type_id:?} owns no namespace scope, got {other:?}"),
    }
}

/// Intrinsic constants such as `i8::MAX` live in a scope hanging off the built-in's symbol, so
/// they are reachable through that scope and through nothing else. A section scope must not see
/// them, otherwise a bare `MAX` would resolve and the ten namespaces would collide on one name.
#[test]
fn scope_core_type_namespace_test() {
    let (compiler, _) = ns_resolve("let CONSTANT = 3");
    let max_id = InternedId::new(intern::INTERNED_MAX_UPPER);
    let min_id = InternedId::new(intern::INTERNED_MIN_UPPER);

    for (name, type_id) in namespaced_builtins() {
        let scope_id = ns_scope_of(&compiler, type_id);

        for bound_id in [max_id, min_id] {
            let found = find_sym_id(
                &compiler,
                AssociatedScopeKind::Scope(scope_id),
                bound_id,
                ScopeType::Core,
                ScopeLookupPattern::NamespaceOnly,
                ScopeLookupPreferenceFlags::none(),
            )
            .unwrap_or_else(|| panic!("`{name}` should hold {bound_id:?} in {scope_id:?}"));

            assert_eq!(found.scope_found_in, scope_id);
            assert!(
                matches!(compiler.symbols[found.found_sym_id].kind, SymbolKind::Variable(_)),
                "`{name}::{bound_id:?}` should be a variable"
            );
        }
    }

    // The same identifier from the module's own scopes, which is what a bare `MAX` searches
    for bound_id in [max_id, min_id] {
        assert!(
            find_sym_id(
                &compiler,
                AssociatedScopeKind::Module(ModuleId::new(0)),
                bound_id,
                ScopeType::Neutral,
                ScopeLookupPattern::NoRestrictions,
                ScopeLookupPreferenceFlags::none(),
            )
            .is_none(),
            "{bound_id:?} must not be reachable without naming the builtin that owns it"
        );
    }
}

/// A bare `MAX` names nothing, and a built-in with no intrinsic namespace has nothing to
/// traverse into. Both must be diagnosed rather than resolving to some other built-in's bound.
#[test]
fn scope_core_type_namespace_unreachable_test() {
    let diags = resolve_single_module("let CONSTANT = MAX", Stage::Constraint);
    assert!(
        diags.err_count() > 0,
        "a bare `MAX` should not resolve: {:?}",
        diags.ty
    );

    // `u64`, `i128`, `u128` and `f128` bounds do not fit `InstiationValue`, and `sized`/`unsized`
    // are pointer-sized, so none of them carry a namespace yet
    for (interned, builtin_ty, ns) in &CORE_BUILTIN_TYPES_DATASET {
        if !ns.is_empty() {
            continue;
        }

        let interner = Intern::init();
        let name = interner.search_idx(*interned as usize);
        let text = format!("let CONSTANT = {name}::MAX");
        let res = resolve_single_module(&text, Stage::Constraint);

        assert!(
            res.err_count() > 0,
            "`{:?}` declares no namespace, so `{name}::MAX` should not resolve",
            builtin_ty.kind()
        );
    }
}
