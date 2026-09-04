use chrn_utils::{
    arena::Arena,
    id_types::{InternedId, ScopeId, TypeId, VariableId},
    intern::{self, Intern},
    source_map::source_span::SourceSpan,
    utils::containers::SpannedContainer,
};
use lang::{
    types::builtins::{BuiltinType, BuiltinTypeKind},
    values::Value,
};

use crate::{
    lookup::scopes::scopes_concepts::{AssociatedScopeKind, ScopeType},
    module::module_concepts::{Import, ImportKind, Module},
    script_compiler::{
        ScriptCompiler,
        compiler_constants::{CORE_UNKNOWN, builtin_ty_to_id},
        helpers::{
            compiler_helpers::DIRECTIVES_DATASET,
            core_helpers::{
                CORE_BOUNDARIES_DATASET, CORE_BUILTIN_TYPES_DATASET, CORE_FUNCS_DATASET,
                core_instantiation_reservations, count_instantiation_bases,
            },
            instantiation_symbols::{
                InstantiationSymbolBase, InstantiationSymbolKind, InstantiationVariable,
                InstiationType, InstiationValue,
            },
        },
    },
    semantic::hir::{
        hir_concepts::{Type, TypeInfo},
        hir_exprs::{ExprHir, ResolvedExpr, ResolvedExprMetadata},
        hir_symbols::{Symbol, SymbolKind, SymbolOrigin, VarDef, VariableMetadata, VariableState},
        value_info::ValueInfo,
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

    for (idx, (interned, builtin_ty, _ns)) in CORE_BUILTIN_TYPES_DATASET.iter().cloned().enumerate()
    {
        // The dataset is loaded sequentially, so an entry's index is the `TypeId` it gets, and the
        // `CORE_*` constant behind `builtin_ty_to_id` must name that same id
        let core_id = idx as u32;
        assert_eq!(
            builtin_ty_to_id(builtin_ty.kind()),
            core_id,
            "CORE constant for {:?} does not match its position in the table",
            builtin_ty.kind()
        );

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

        let sym = &compiler.syms[info.sym_id];
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
            .syms
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

        let sym = &compiler.syms[sym_id];

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
    let ns_counts = core_instantiation_reservations();

    let expected_type_count = CORE_BUILTIN_TYPES_DATASET.len()
        + CORE_BOUNDARIES_DATASET.len()
        + CORE_FUNCS_DATASET.len()
        + 1;
    assert_eq!(compiler.types.len(), expected_type_count);
    assert_eq!(compiler.types.capacity(), expected_type_count);

    // A module binds its own name plus one identifier per import -- the alias when given, the file
    // name otherwise. An identifier already bound in the module's scope is filtered out, which is
    // what happens to the implicit `core` import inside the `core` module itself.
    let expected_module_symbol_count: usize = compiler
        .mods
        .iter()
        .map(|module| {
            let mut idents: Vec<InternedId> = vec![module.name_id];
            for import in &module.imports {
                let ident = import
                    .sp_alias_id
                    .as_ref()
                    .map(|alias| alias.inner)
                    .unwrap_or(import.name_id);
                if !idents.contains(&ident) {
                    idents.push(ident);
                }
            }
            idents.len()
        })
        .sum();
    let expected_symbol_count = CORE_BUILTIN_TYPES_DATASET.len()
        + CORE_BOUNDARIES_DATASET.len()
        + CORE_FUNCS_DATASET.len()
        + DIRECTIVES_DATASET.len()
        + expected_module_symbol_count
        + ns_counts.symbols;
    assert_eq!(compiler.syms.len(), expected_symbol_count);
    assert_eq!(compiler.syms.capacity(), expected_symbol_count);

    assert_eq!(compiler.directives.len(), DIRECTIVES_DATASET.len());
    assert_eq!(compiler.directives.capacity(), DIRECTIVES_DATASET.len());

    // `load_core` creates one core scope plus a scope for every built-in carrying an intrinsic
    // namespace; module-symbol registration creates one compiler scope for every module, including
    // the implicit core module.
    let expected_scope_count = compiler.mods.len() + 1 + ns_counts.scopes;
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
        Some(SpannedContainer::new(
            InternedId::new(1),
            SourceSpan::default(),
        )),
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
    let ns_counts = core_instantiation_reservations();

    // A module binds its own name plus one identifier per import -- the alias when given, the file
    // name otherwise. An identifier already bound in the module's scope is filtered out, which is
    // what happens to the implicit `core` import inside the `core` module itself.
    let expected_module_symbol_count: usize = compiler
        .mods
        .iter()
        .map(|module| {
            let mut idents: Vec<InternedId> = vec![module.name_id];
            for import in &module.imports {
                let ident = import
                    .sp_alias_id
                    .as_ref()
                    .map(|alias| alias.inner)
                    .unwrap_or(import.name_id);
                if !idents.contains(&ident) {
                    idents.push(ident);
                }
            }
            idents.len()
        })
        .sum();
    let expected_symbol_count = CORE_BUILTIN_TYPES_DATASET.len()
        + CORE_BOUNDARIES_DATASET.len()
        + CORE_FUNCS_DATASET.len()
        + DIRECTIVES_DATASET.len()
        + expected_module_symbol_count
        + ns_counts.symbols;

    assert_eq!(compiler.syms.len(), expected_symbol_count);
    assert_eq!(compiler.syms.capacity(), expected_symbol_count);
    let expected_scope_count = compiler.mods.len() + 1 + ns_counts.scopes;
    assert_eq!(compiler.scopes.len(), expected_scope_count);
    assert_eq!(compiler.scopes.capacity(), expected_scope_count);
}

// -- Intrinsic namespaces --

/// Both halves of a `MAX`/`MIN` pair, as they appear in the compiler after `load_core`.
struct RegisteredConstant<'a> {
    sym: &'a Symbol,
    var: &'a VarDef,
    val: &'a ValueInfo,
    expr: &'a ResolvedExpr,
}

/// Looks up `name_id` in the scope a namespaced built-in owns and gathers everything
/// `register_instantiation_var` pushed for it.
fn registered_constant<'a>(
    compiler: &'a ScriptCompiler,
    scope_id: ScopeId,
    name_id: InternedId,
) -> RegisteredConstant<'a> {
    let sym_id = compiler.get_scope(scope_id).scope.table.interned_to_sym[&name_id];
    let sym = &compiler.syms[sym_id];

    let SymbolKind::Variable(var_id) = sym.kind else {
        panic!(
            "`{name_id:?}` in {scope_id:?} is not a variable, got {:?}",
            sym.kind
        );
    };

    let var = &compiler.variables[var_id];

    let VariableState::Known(val_id) = var.state else {
        panic!("`{name_id:?}` in {scope_id:?} has no known value");
    };

    let val = &compiler.values[val_id];
    let expr = &compiler.exprs[val.expr_id];

    RegisteredConstant {
        sym,
        var,
        val,
        expr,
    }
}

/// The scope a namespaced built-in owns, or `None` when the entry declares no namespace.
/// Panics when the two disagree, since `register_builtin` derives one from the other.
fn builtin_ns_scope(
    compiler: &ScriptCompiler,
    type_id: TypeId,
    ns: &[InstantiationSymbolBase],
) -> Option<ScopeId> {
    let Type::BuiltinTypeInfo(info) = &compiler.types[type_id].ty else {
        panic!(
            "expected a builtin at {type_id:?}, got {:?}",
            compiler.types[type_id].ty
        );
    };

    match compiler.syms[info.sym_id].associated_scope {
        Some(AssociatedScopeKind::Scope(scope_id)) => {
            assert!(
                !ns.is_empty(),
                "builtin at {type_id:?} declares no namespace but owns {scope_id:?}"
            );
            Some(scope_id)
        }
        None => {
            assert!(
                ns.is_empty(),
                "builtin at {type_id:?} declares a namespace but owns no scope"
            );
            None
        }
        other => panic!("builtin at {type_id:?} has a non-scope association: {other:?}"),
    }
}

/// `MAX` or `MIN`, for messages. Every intrinsic namespace entry is one of the two.
fn bound_name(name_id: InternedId) -> &'static str {
    match name_id.id {
        intern::INTERNED_MAX_UPPER => "MAX",
        intern::INTERNED_MIN_UPPER => "MIN",
        other => {
            panic!("intrinsic namespaces only hold `MAX` and `MIN`, found interned id {other}")
        }
    }
}

/// `Value` has no `PartialEq`, and only the variants `InstiationValue` can produce are compared.
fn same_value(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::I64(l), Value::I64(r)) => l == r,
        // Bit equality rather than `==` so a bound that lost precision on the way in fails
        (Value::F64(l), Value::F64(r)) => l.to_bits() == r.to_bits(),
        (Value::Bool(l), Value::Bool(r)) => l == r,
        (Value::Char(l), Value::Char(r)) => l == r,
        (Value::InternedStr(l), Value::InternedStr(r)) => l == r,
        _ => false,
    }
}

/// The bound a built-in's `MAX` and `MIN` must carry, as `(max, min)`. Spelled out here rather
/// than derived from the dataset so a wrong constant in the dataset fails instead of agreeing
/// with itself.
fn expected_bounds(kind: BuiltinTypeKind) -> Option<(Value, Value)> {
    let pair = match kind {
        BuiltinTypeKind::I8 => (Value::I64(127), Value::I64(-128)),
        BuiltinTypeKind::U8 => (Value::I64(255), Value::I64(0)),
        BuiltinTypeKind::I16 => (Value::I64(32_767), Value::I64(-32_768)),
        BuiltinTypeKind::U16 => (Value::I64(65_535), Value::I64(0)),
        BuiltinTypeKind::F16 => (Value::F64(65_504.0), Value::F64(-65_504.0)),
        BuiltinTypeKind::I32 => (Value::I64(2_147_483_647), Value::I64(-2_147_483_648)),
        BuiltinTypeKind::U32 => (Value::I64(4_294_967_295), Value::I64(0)),
        BuiltinTypeKind::F32 => (Value::F64(f32::MAX as f64), Value::F64(f32::MIN as f64)),
        BuiltinTypeKind::I64 => (Value::I64(i64::MAX), Value::I64(i64::MIN)),
        BuiltinTypeKind::F64 => (Value::F64(f64::MAX), Value::F64(f64::MIN)),
        // `u64`, `i128`, `u128` and `f128` do not fit `InstiationValue`, and `sized`/`unsized`
        // are pointer-sized, so their bounds belong to the target rather than the host
        _ => return None,
    };

    Some(pair)
}

#[test]
fn core_namespaced_builtins_own_a_core_scope() {
    let compiler = core_only_compiler();
    let core_mod_id = compiler.intrinsic_registry.core_mod_id;

    for (idx, (_, builtin_ty, ns)) in CORE_BUILTIN_TYPES_DATASET.iter().enumerate() {
        let type_id = TypeId::new(idx as u32);
        let Some(scope_id) = builtin_ns_scope(&compiler, type_id, ns) else {
            continue;
        };

        let Type::BuiltinTypeInfo(info) = &compiler.types[type_id].ty else {
            unreachable!("checked by `builtin_ns_scope`");
        };
        let scope_info = compiler.get_scope(scope_id);

        assert_eq!(
            scope_info.sym_owner,
            Some(info.sym_id),
            "namespace scope of {:?} is not owned by its builtin's symbol",
            builtin_ty.kind()
        );
        assert_eq!(
            scope_info.mod_owner,
            core_mod_id,
            "namespace scope of {:?} is not owned by the core module",
            builtin_ty.kind()
        );
        assert_eq!(
            scope_info.scope.scope_type,
            ScopeType::Core,
            "namespace scope of {:?} is not a core scope",
            builtin_ty.kind()
        );
        assert_eq!(
            scope_info.scope.table.interned_to_sym.len(),
            ns.len(),
            "namespace scope of {:?} does not hold exactly its dataset entries",
            builtin_ty.kind()
        );
    }
}

#[test]
fn core_namespaces_declare_max_and_min() {
    for (_, builtin_ty, ns) in &CORE_BUILTIN_TYPES_DATASET {
        if ns.is_empty() {
            continue;
        }

        let names: Vec<u32> = ns.iter().map(|base| base.name_id.id).collect();

        assert_eq!(
            names,
            vec![intern::INTERNED_MAX_UPPER, intern::INTERNED_MIN_UPPER],
            "namespace of {:?} is not exactly `MAX` then `MIN`",
            builtin_ty.kind()
        );
    }
}

#[test]
fn core_namespace_entries_are_typed_as_their_builtin() {
    for (_, builtin_ty, ns) in &CORE_BUILTIN_TYPES_DATASET {
        for base in ns.iter() {
            let InstantiationSymbolKind::Variable(var) = &base.kind else {
                panic!(
                    "namespace of {:?} holds a non-variable entry: {:?}",
                    builtin_ty.kind(),
                    base.kind
                );
            };

            let InstiationType::BuiltinType(entry_ty) = &var.ty;

            // A copy-pasted namespace that kept the type of the entry it was copied from would
            // register `i16::MAX` as an `i8`, which nothing downstream would flag
            assert_eq!(
                entry_ty.kind(),
                builtin_ty.kind(),
                "namespace of {:?} declares an entry typed {:?}",
                builtin_ty.kind(),
                entry_ty.kind()
            );
        }
    }
}

#[test]
fn core_namespace_bounds_match_target_limits() {
    for (_, builtin_ty, ns) in &CORE_BUILTIN_TYPES_DATASET {
        let Some((expected_max, expected_min)) = expected_bounds(builtin_ty.kind()) else {
            assert!(
                ns.is_empty(),
                "{:?} carries a namespace with no expected bounds",
                builtin_ty.kind()
            );
            continue;
        };

        assert_eq!(
            ns.len(),
            2,
            "{:?} has target bounds but no namespace holding them",
            builtin_ty.kind()
        );

        for (base, expected) in ns.iter().zip([expected_max, expected_min]) {
            let InstantiationSymbolKind::Variable(var) = &base.kind else {
                unreachable!("checked by `core_namespace_entries_are_typed_as_their_builtin`");
            };

            let found = var.val.to_val();

            assert!(
                same_value(&found, &expected),
                "{:?}::{} is {found:?}, expected {expected:?}",
                builtin_ty.kind(),
                bound_name(base.name_id)
            );
        }
    }
}

#[test]
fn core_namespace_constants_are_registered_as_variables() {
    let compiler = core_only_compiler();

    for (idx, (_, builtin_ty, ns)) in CORE_BUILTIN_TYPES_DATASET.iter().enumerate() {
        let type_id = TypeId::new(idx as u32);
        let Some(scope_id) = builtin_ns_scope(&compiler, type_id, ns) else {
            continue;
        };

        for base in ns.iter() {
            let InstantiationSymbolKind::Variable(var) = &base.kind else {
                unreachable!("checked by `core_namespace_entries_are_typed_as_their_builtin`");
            };

            let found = registered_constant(&compiler, scope_id, base.name_id);
            let label = format!("{:?}::{}", builtin_ty.kind(), bound_name(base.name_id));

            assert_eq!(found.sym.name_id, base.name_id, "symbol name of {label}");
            // `SymbolOrigin` carries a `ModuleId` and has no `PartialEq`
            let origins_match = match (found.sym.sym_origin, base.sym_origin) {
                (SymbolOrigin::Compiler, SymbolOrigin::Compiler) => true,
                (SymbolOrigin::Module(found_id), SymbolOrigin::Module(declared)) => {
                    found_id == declared
                }
                _ => false,
            };
            assert!(
                origins_match,
                "origin of {label} is {:?}, dataset declares {:?}",
                found.sym.sym_origin, base.sym_origin
            );
            assert_eq!(found.sym.is_priv, base.is_priv, "privacy of {label}");
            assert!(
                found.sym.ast_id.is_none(),
                "{label} is compiler generated and must have no ast id"
            );
            assert!(
                found.sym.associated_scope.is_none(),
                "{label} is a value and must own no scope"
            );

            assert_eq!(found.var.sym_id, found.sym.sym_id, "var of {label}");
            assert_eq!(found.var.name_id, base.name_id, "var name of {label}");
            assert!(
                matches!(found.var.meta, VariableMetadata::Generated),
                "{label} is compiler generated"
            );

            // The constant is typed as the built-in it hangs off, not as whatever its `Value`
            // payload happens to be -- `u8::MAX` is a `u8` holding an `I64`
            assert_eq!(
                found.val.type_id, type_id,
                "{label} is not typed as its own builtin"
            );

            let expected = var.val.to_val();
            let const_val = found
                .val
                .const_val
                .as_ref()
                .unwrap_or_else(|| panic!("{label} has no constant value"));

            assert!(
                same_value(const_val, &expected),
                "{label} is {const_val:?}, expected {expected:?}"
            );

            // `register_instantiation_var` pushes the value and its expression as a pair, and
            // later stages follow the link in both directions
            assert_eq!(
                found.expr.type_id, found.val.type_id,
                "expr and value of {label} disagree on the type"
            );
            assert!(
                matches!(found.expr.expr_hir, ExprHir::Val(val_id) if val_id == found.expr.val_id),
                "expr of {label} does not hold its own value"
            );
            assert!(
                matches!(found.expr.meta, ResolvedExprMetadata::Generated),
                "expr of {label} is compiler generated"
            );
            assert_eq!(
                compiler.values[found.expr.val_id].expr_id, found.val.expr_id,
                "value and expr of {label} do not point at each other"
            );
        }
    }
}

#[test]
fn core_namespace_constants_stay_out_of_the_core_scope() {
    let compiler = core_only_compiler();
    let core_mod_id = compiler.intrinsic_registry.core_mod_id;
    let core_scope_id = compiler.extract_scope_id(ScopeType::Core, core_mod_id);
    let table = &compiler.get_scope(core_scope_id).scope.table;

    // `MAX` and `MIN` are only reachable through the built-in that owns them. Leaking them into
    // the core scope would make a bare `MAX` resolve, and would collide across the ten namespaces
    for interned in [intern::INTERNED_MAX_UPPER, intern::INTERNED_MIN_UPPER] {
        assert!(
            !table
                .interned_to_sym
                .contains_key(&InternedId::new(interned)),
            "core scope must not bind the intrinsic constant at interned id {interned}"
        );
    }

    let core_mod = &compiler.mods[core_mod_id];
    assert_eq!(
        core_mod.exports.len(),
        table.interned_to_sym.len(),
        "core exports must be exactly what the core scope binds"
    );
}

#[test]
fn core_namespace_arenas_are_reserved_exactly() {
    let compiler = core_only_compiler();
    let ns_counts = core_instantiation_reservations();

    // One `VarDef`, one `ResolvedExpr`, and one `ValueInfo` per constant, and nothing else has
    // been pushed yet, so the arenas are exactly as long as they were reserved
    for (label, len, capacity) in [
        (
            "variables",
            compiler.variables.len(),
            compiler.variables.capacity(),
        ),
        ("exprs", compiler.exprs.len(), compiler.exprs.capacity()),
        ("values", compiler.values.len(), compiler.values.capacity()),
    ] {
        assert_eq!(len, ns_counts.variables, "{label} length");
        assert_eq!(capacity, ns_counts.variables, "{label} capacity");
    }

    for (idx, var) in compiler.variables.iter().enumerate() {
        let var_id = VariableId::new(idx as u32);
        let sym = &compiler.syms[var.sym_id];

        assert!(
            matches!(sym.kind, SymbolKind::Variable(found) if found == var_id),
            "variable {idx} and its symbol do not point at each other"
        );
    }
}

// -- `count_instantiation_bases` --

static COUNT_LEAF_DATASET: [InstantiationSymbolBase; 2] = [
    InstantiationSymbolBase::new(
        InternedId::new(intern::INTERNED_MAX_UPPER),
        SymbolOrigin::Compiler,
        ScopeType::Compiler,
        false,
        InstantiationSymbolKind::Variable(InstantiationVariable::new(
            InstiationType::BuiltinType(BuiltinType::I8),
            InstiationValue::I64(1),
        )),
    ),
    InstantiationSymbolBase::new(
        InternedId::new(intern::INTERNED_INT),
        SymbolOrigin::Compiler,
        ScopeType::Compiler,
        false,
        InstantiationSymbolKind::ExternType,
    ),
];

static COUNT_ROOT_DATASET: [InstantiationSymbolBase; 2] = [
    InstantiationSymbolBase::new(
        InternedId::new(intern::INTERNED_TYPES_LOWER),
        SymbolOrigin::Compiler,
        ScopeType::Compiler,
        false,
        InstantiationSymbolKind::Namespace(&COUNT_LEAF_DATASET),
    ),
    InstantiationSymbolBase::new(
        InternedId::new(intern::INTERNED_MIN_UPPER),
        SymbolOrigin::Compiler,
        ScopeType::Compiler,
        false,
        InstantiationSymbolKind::Variable(InstantiationVariable::new(
            InstiationType::BuiltinType(BuiltinType::I8),
            InstiationValue::I64(0),
        )),
    ),
];

/// The core dataset is flat, so nesting is only covered here. `register_instantiation_bases`
/// descends into a namespace, which the reservation counts must follow.
#[test]
fn count_instantiation_bases_descends_into_namespaces() {
    let counts = count_instantiation_bases(&COUNT_ROOT_DATASET);

    // Namespace, its two members, and the root variable
    assert_eq!(counts.symbols, 4);
    // Only the nested namespace owns one; the root's own scope belongs to its caller
    assert_eq!(counts.scopes, 1);
    // The extern type contributes no value
    assert_eq!(counts.variables, 2);
}
