use chrn_utils::{
    arena::Arena,
    id_types::{InternedId, TypeId},
    intern::Intern,
};

use crate::{
    lookup::scopes::scopes_concepts::ScopeType,
    modules::{Import, ImportKind, Module},
    script_compiler::{
        CORE_UNKNOWN, ScriptCompiler,
        helpers::compiler_helpers::DIRECTIVES_DATASET,
        helpers::core_helpers::{
            CORE_BOUNDARIES_DATASET, CORE_BUILTIN_TYPES_DATASET, CORE_FUNCS_DATASET,
        },
    },
    semantic::hir::{
        hir_concepts::{Type, TypeInfo},
        hir_symbols::SymbolKind,
    },
};

/// Builds the core module the same way `ScriptCompiler::init` does, with no user modules
fn core_only_compiler() -> ScriptCompiler {
    ScriptCompiler::init(None, Arena::new())
}

#[test]
fn core_builtin_type_id_alignment() {
    let compiler = core_only_compiler();
    let interner = Intern::init();

    for (interned, builtin_ty, core_id) in CORE_BUILTIN_TYPES_DATASET.iter().cloned() {
        let type_info: &TypeInfo = &compiler.types[TypeId::new(core_id)];

        let Type::BuiltinTypeInfo(info) = &type_info.ty else {
            panic!(
                "expected builtin type at CORE id {core_id}, got {:?}",
                type_info.ty
            );
        };

        assert_eq!(
            info.ty.kind(),
            builtin_ty.kind(),
            "builtin at CORE id {core_id} does not match the table entry"
        );

        let sym = &compiler.symbols[info.sym_id];
        assert_eq!(
            sym.name_id,
            InternedId::new(interned),
            "symbol name for {} does not match its interned id",
            interner.search_idx(interned as usize)
        );
    }
}

#[test]
fn core_unknown_type_id_alignment() {
    let compiler = core_only_compiler();
    let type_info = &compiler.types[TypeId::new(CORE_UNKNOWN)];

    assert!(
        matches!(type_info.ty, Type::Unknown),
        "expected Type::Unknown at CORE_UNKNOWN, got {:?}",
        type_info.ty
    );
}

#[test]
fn core_boundaries_follow_builtins() {
    let compiler = core_only_compiler();

    // Boundaries are pushed directly after the unknown type
    let first = CORE_UNKNOWN + 1;

    for (idx, (interned, flags)) in CORE_BOUNDARIES_DATASET.into_iter().enumerate() {
        let type_id = TypeId::new(first + idx as u32);
        let type_info = &compiler.types[type_id];

        let Type::Boundaries(found) = type_info.ty else {
            panic!(
                "expected boundary type at {type_id:?}, got {:?}",
                type_info.ty
            );
        };

        assert_eq!(found, flags, "boundary at {type_id:?} does not match");

        let sym = compiler
            .symbols
            .items
            .iter()
            .find(|sym| sym.name_id == InternedId::new(interned))
            .expect("boundary symbol should exist");

        assert!(
            matches!(sym.kind, SymbolKind::Type(id) if id == type_id),
            "boundary symbol does not point at its type"
        );
    }
}

/// `TypeId` of the first core function, which is pushed after the builtins, the unknown type and
/// the boundaries
fn first_core_func_type_id() -> u32 {
    CORE_UNKNOWN + 1 + CORE_BOUNDARIES_DATASET.len() as u32
}

#[test]
fn core_funcs_follow_boundaries() {
    let compiler = core_only_compiler();
    let first = first_core_func_type_id();

    for (idx, core_func) in CORE_FUNCS_DATASET.iter().enumerate() {
        let type_id = TypeId::new(first + idx as u32);
        let type_info = &compiler.types[type_id];

        let Type::Func(func_def) = &type_info.ty else {
            panic!("expected func type at {type_id:?}, got {:?}", type_info.ty);
        };

        assert_eq!(func_def.kind, core_func.kind, "kind at {type_id:?}");
        assert_eq!(
            func_def.name_id,
            InternedId::new(core_func.name),
            "name at {type_id:?}"
        );
        assert_eq!(
            func_def.is_callable, core_func.is_callable,
            "is_callable at {type_id:?}"
        );
        assert_eq!(
            func_def.affects_type_constraint, core_func.affects_type_constraint,
            "affects_type_constraint at {type_id:?}"
        );
        assert_eq!(
            func_def.type_constraints, core_func.type_constraints,
            "type_constraints at {type_id:?}"
        );
        assert_eq!(
            func_def.arg_constraints, core_func.arg_constraints,
            "arg_constraints at {type_id:?}"
        );
        assert_eq!(
            func_def.ret_type,
            TypeId::new(core_func.ret_type),
            "ret_type at {type_id:?}"
        );
    }
}

#[test]
fn core_funcs_are_symbols_in_core_scope() {
    let compiler = core_only_compiler();
    let first = first_core_func_type_id();

    let core_mod_id = compiler.intrinsic_registry.core_mod_id;
    let core_scope_id = compiler.extract_scope_id(ScopeType::Core, core_mod_id);
    let table = &compiler.get_scope(core_scope_id).scope.table;

    for (idx, core_func) in CORE_FUNCS_DATASET.iter().enumerate() {
        let type_id = TypeId::new(first + idx as u32);
        let name_id = InternedId::new(core_func.name);

        let sym_id = *table
            .interned_to_sym
            .get(&name_id)
            .unwrap_or_else(|| panic!("core func {:?} is not in the core scope", core_func.kind));

        let sym = &compiler.symbols[sym_id];

        assert_eq!(sym.name_id, name_id, "symbol name for {:?}", core_func.kind);
        assert!(
            matches!(sym.kind, SymbolKind::Type(id) if id == type_id),
            "symbol for {:?} does not point at {type_id:?}",
            core_func.kind
        );

        let Type::Func(func_def) = &compiler.types[type_id].ty else {
            panic!("expected func type at {type_id:?}");
        };

        assert_eq!(
            func_def.sym_id, sym_id,
            "func def for {:?} does not point back at its symbol",
            core_func.kind
        );
    }
}

#[test]
fn core_func_return_types_are_loaded() {
    let compiler = core_only_compiler();

    // Return types are indexes into types loaded before the funcs themselves
    for core_func in &CORE_FUNCS_DATASET {
        assert!(
            core_func.ret_type < first_core_func_type_id(),
            "return type of {:?} is not loaded before the funcs",
            core_func.kind
        );

        let type_info = &compiler.types[TypeId::new(core_func.ret_type)];
        assert!(
            matches!(type_info.ty, Type::BuiltinTypeInfo(_)),
            "return type of {:?} is not a builtin, got {:?}",
            core_func.kind,
            type_info.ty
        );
    }
}

#[test]
fn startup_reservations_match_loaded_data() {
    let compiler = core_only_compiler();

    let expected_type_count = CORE_BUILTIN_TYPES_DATASET.len()
        + CORE_BOUNDARIES_DATASET.len()
        + CORE_FUNCS_DATASET.len()
        + 1;
    assert_eq!(compiler.types.len(), expected_type_count);
    assert_eq!(compiler.types.capacity(), expected_type_count);

    let expected_module_symbol_count: usize = compiler
        .mods
        .iter()
        .map(|module| {
            1 + module.imports.len()
                + module
                    .imports
                    .iter()
                    .filter(|import| import.alias_id.is_some())
                    .count()
        })
        .sum();
    let expected_symbol_count = CORE_BUILTIN_TYPES_DATASET.len()
        + CORE_BOUNDARIES_DATASET.len()
        + CORE_FUNCS_DATASET.len()
        + DIRECTIVES_DATASET.len()
        + expected_module_symbol_count;
    assert_eq!(compiler.symbols.len(), expected_symbol_count);
    assert_eq!(compiler.symbols.capacity(), expected_symbol_count);

    assert_eq!(compiler.directives.len(), DIRECTIVES_DATASET.len());
    assert_eq!(compiler.directives.capacity(), DIRECTIVES_DATASET.len());

    // `load_core` creates one core scope; module-symbol registration creates one compiler scope
    // for every module, including the implicit core module.
    let expected_scope_count = compiler.mods.len() + 1;
    assert_eq!(compiler.scopes.len(), expected_scope_count);
    assert_eq!(compiler.scopes.capacity(), expected_scope_count);

    let core_mod = &compiler.mods[compiler.intrinsic_registry.core_mod_id];
    let expected_core_exports =
        CORE_BUILTIN_TYPES_DATASET.len() + CORE_BOUNDARIES_DATASET.len() + CORE_FUNCS_DATASET.len();
    assert_eq!(core_mod.exports.len(), expected_core_exports);
    assert_eq!(core_mod.exports.capacity(), expected_core_exports);
}

#[test]
fn startup_reservations_include_user_module_symbols() {
    let import = Import::new(
        InternedId::new(0),
        ImportKind::Core(Default::default()),
        Some(InternedId::new(1)),
    );
    let module = Module::new(
        InternedId::new(2),
        Default::default(),
        Default::default(),
        None,
        vec![import],
        None,
    );
    let compiler = ScriptCompiler::init(None, Arena::from(vec![module]));

    let expected_module_symbol_count: usize = compiler
        .mods
        .iter()
        .map(|module| {
            1 + module.imports.len()
                + module
                    .imports
                    .iter()
                    .filter(|import| import.alias_id.is_some())
                    .count()
        })
        .sum();
    let expected_symbol_count = CORE_BUILTIN_TYPES_DATASET.len()
        + CORE_BOUNDARIES_DATASET.len()
        + CORE_FUNCS_DATASET.len()
        + DIRECTIVES_DATASET.len()
        + expected_module_symbol_count;

    assert_eq!(compiler.symbols.len(), expected_symbol_count);
    assert_eq!(compiler.symbols.capacity(), expected_symbol_count);
    assert_eq!(compiler.scopes.len(), compiler.mods.len() + 1);
    assert_eq!(compiler.scopes.capacity(), compiler.mods.len() + 1);
}
