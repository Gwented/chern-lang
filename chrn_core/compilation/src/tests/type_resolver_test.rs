use super::helpers::*;
use crate::script_compiler::compiler_constants::{CORE_I64, CORE_STR, CORE_UNKNOWN};
use crate::semantic::hir::hir_concepts::Type;
use crate::semantic::hir::hir_exprs::ResolvedExpr;
use crate::semantic::hir::hir_impls::ImplMemberKind;
use crate::semantic::hir::hir_symbols::SymbolKind;
use crate::semantic::hir::value_info::ValueInfo;
use crate::walk_type_id_deferred;
use chrn_utils::id_types::TypeId;

#[test]
fn type_resolver_simple_test() {
    let wrong = "
            var->
                primitive: i32
                undeclared_type: Thing
            ";

    let resolution = resolve_single_module(wrong, Stage::Type);
    let res = &resolution.ty;
    let compiler = &resolution.compiler;
    let interner = &resolution.interner;

    let err = &res.diags;
    assert_eq!(
        err.len(),
        1,
        "Expected exactly one diagnostic for undeclared type"
    );
    assert_eq!(err[0].level, DiagnosticLevel::Error);
    assert!(
        err[0].core_msg.contains("Thing"),
        "Error should mention 'Thing': {}",
        err[0].core_msg
    );

    // The undeclared type's typedef inner type should remain unknown
    let undeclared_sym = compiler
        .symbols
        .iter()
        .find(|s| interner.search(s.name_id) == "undeclared_type")
        .expect("undeclared_type symbol should exist");
    let undeclared_type_id = match &undeclared_sym.kind {
        SymbolKind::Type(type_id) => *type_id,
        other => panic!("undeclared_type symbol should be Type, got {:?}", other),
    };
    let undeclared_ty = &compiler.types[undeclared_type_id].ty;
    match undeclared_ty {
        Type::TypeDef(type_def) => {
            assert_eq!(
                type_def.type_id,
                TypeId::new(CORE_UNKNOWN),
                "undeclared_type should resolve to unknown"
            );
        }
        other => panic!("Expected undeclared_type to be a TypeDef, got {:?}", other),
    }

    // primitive should still resolve correctly despite the error
    let primitive_sym = compiler
        .symbols
        .iter()
        .find(|s| interner.search(s.name_id) == "primitive")
        .expect("primitive symbol should exist");
    let primitive_type_id = match &primitive_sym.kind {
        SymbolKind::Type(type_id) => *type_id,
        other => panic!("primitive symbol should be Type, got {:?}", other),
    };
    let primitive_ty = &compiler.types[primitive_type_id].ty;
    match primitive_ty {
        Type::TypeDef(type_def) => {
            assert!(
                !compiler.check_unknown(type_def.type_id),
                "primitive should resolve to a known type, not unknown"
            );
        }
        other => panic!("Expected primitive to be a TypeDef, got {:?}", other),
    }

    let correct = "
            var->
                primitive: i32
                declared_type: Thing
            nest->
                struct Thing {}
            ";

    let resolution = resolve_single_module(correct, Stage::Type);
    let res = &resolution.ty;
    let compiler = &resolution.compiler;
    let interner = &resolution.interner;
    dbg!(&res);

    assert!(res.err_count() == 0, "Type resolution should succeed");

    // Verify Thing is a struct with no fields
    let thing_sym = compiler
        .symbols
        .iter()
        .find(|s| interner.search(s.name_id) == "Thing")
        .expect("Thing symbol should exist");
    let thing_type_id = match &thing_sym.kind {
        SymbolKind::Type(type_id) => *type_id,
        other => panic!("Thing symbol should be Type, got {:?}", other),
    };
    let thing_type = &compiler.types[thing_type_id].ty;
    let thing_fields = match thing_type {
        Type::Struct(struct_def) => &struct_def.fields,
        other => panic!("Expected Thing to be a struct, got {:?}", other),
    };
    assert!(
        thing_fields.is_empty(),
        "Thing struct should have no fields"
    );

    // Verify primitive typedef resolves to a known type (i32)
    let primitive_sym = compiler
        .symbols
        .iter()
        .find(|s| interner.search(s.name_id) == "primitive")
        .expect("primitive symbol should exist");
    let primitive_type_id = match &primitive_sym.kind {
        SymbolKind::Type(type_id) => *type_id,
        other => panic!("primitive symbol should be Type, got {:?}", other),
    };
    let primitive_ty = &compiler.types[primitive_type_id].ty;
    match primitive_ty {
        Type::TypeDef(type_def) => {
            assert!(
                !compiler.check_unknown(type_def.type_id),
                "primitive should resolve to a known type"
            );
        }
        other => panic!("Expected primitive to be a TypeDef, got {:?}", other),
    }

    // Verify declared_type typedef resolves to Thing's struct type
    let declared_type_sym = compiler
        .symbols
        .iter()
        .find(|s| interner.search(s.name_id) == "declared_type")
        .expect("declared_type symbol should exist");
    let declared_type_id = match &declared_type_sym.kind {
        SymbolKind::Type(type_id) => *type_id,
        other => panic!("declared_type symbol should be Type, got {:?}", other),
    };
    let declared_ty = &compiler.types[declared_type_id].ty;
    match declared_ty {
        Type::TypeDef(type_def) => {
            assert_eq!(
                type_def.type_id, thing_type_id,
                "declared_type should resolve to Thing struct type"
            );
        }
        other => panic!("Expected declared_type to be a TypeDef, got {:?}", other),
    }
}

#[test]
fn type_resolver_complex_test() {
    let text = "
            let CONSTANT = 4
            ";

    let resolution = resolve_single_module(text, Stage::Type);
    let summary = &resolution.ty;
    let compiler = &resolution.compiler;
    let interner = &resolution.interner;
    assert!(summary.err_count() == 0, "Type resolution failed");

    let constant_sym = compiler
        .symbols
        .iter()
        .find(|s| interner.search(s.name_id) == "CONSTANT")
        .expect("CONSTANT symbol should exist");
    let var_id = match &constant_sym.kind {
        SymbolKind::Variable(var_id) => *var_id,
        other => panic!("CONSTANT symbol should be Variable, got {:?}", other),
    };
    let var_def = &compiler.variables[var_id];
    let val_id = match &var_def.state {
        VariableState::Known(val_id) => *val_id,
        other => panic!("CONSTANT should be Known, got {:?}", other),
    };
    let val_info = &compiler.values[val_id];
    assert_eq!(
        val_info.type_id,
        TypeId::new(CORE_I64),
        "CONSTANT should have i64 type"
    );
    assert!(
        matches!(val_info.const_val, Some(Value::I64(4))),
        "CONSTANT should have const value Some(I64(4)), got {:?}",
        val_info.const_val
    );
}

#[test]
fn type_resolver_string_concat_basic_test() {
    let (compiler, interner) = compile_and_resolve_single_module("let X = \"Hello\" + \" World\"");

    let val = value_of(&compiler, &interner, "X");
    match &val {
        Value::InternedStr(id) => {
            assert_eq!(
                interner.search(*id),
                "Hello World",
                "String concat should produce exact value"
            );
        }
        other => panic!("Expected InternedStr, got {:?}", other),
    }
}

#[test]
fn type_resolver_string_concat_type_test() {
    let (compiler, interner) = compile_and_resolve_single_module("let X = \"Hello\" + \" World\"");

    let name_id = interner
        .try_search_str("X")
        .expect("'X' should be interned");
    let var_def = compiler
        .variables
        .iter()
        .find(|v| v.name_id == name_id)
        .expect("Variable 'X' not found");
    let val_id = match &var_def.state {
        VariableState::Known(val_id) => *val_id,
        other => panic!("'X' should be Known, got {:?}", other),
    };
    let val_info = &compiler.values[val_id];

    assert_eq!(
        val_info.type_id,
        TypeId::new(CORE_STR),
        "String concat result should have str type"
    );
}

fn root_option_expr_and_value<'a>(
    resolution: &'a Resolution,
    option_name: &str,
) -> (&'a ResolvedExpr, &'a ValueInfo) {
    let option = resolution
        .compiler
        .impl_members
        .iter()
        .find_map(|member| match member {
            ImplMemberKind::OptAssignmentRoot(option)
                if resolution.interner.search(option.name_id) == option_name =>
            {
                Some(option)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("root option `{option_name}` should exist"));
    let array_expr = &resolution.compiler.exprs[option.array_expr_id];
    (array_expr, &resolution.compiler.values[array_expr.val_id])
}

fn assert_i64_array(option_name: &str, value: &ValueInfo, expected: &[i64]) {
    let Some(Value::Array(values)) = &value.const_val else {
        panic!(
            "option `{option_name}` should hold a constant array, got {:?}",
            value.const_val
        );
    };
    assert_eq!(values.len(), expected.len());
    for (value, expected) in values.iter().zip(expected) {
        assert!(
            matches!(value, Value::I64(found) if found == expected),
            "expected i64 value {expected}, got {value:?}"
        );
    }
}

fn concrete_type(resolution: &Resolution, mut type_id: TypeId) -> TypeId {
    walk_type_id_deferred!(&resolution.compiler.types, type_id).inner
}

#[test]
fn standing_expr_resolves_pending_symbol_in_root_option() {
    let resolution = resolve_single_module(
        "
            let PENDING = SOURCE
            let SOURCE = 4
            nest->
                struct Settings {}
            complex->
                Settings { values = [PENDING] }
        ",
        Stage::Type,
    )
    .expect_ok();

    let (expr, value) = root_option_expr_and_value(&resolution, "values");
    assert_eq!(
        concrete_type(&resolution, expr.type_id),
        TypeId::new(CORE_I64)
    );
    assert_i64_array("values", value, &[4]);
}

#[test]
fn standing_expr_repairs_each_dependent_expression_tree() {
    let resolution = resolve_single_module(
        "
            let PENDING = SOURCE
            let SOURCE = 4
            nest->
                struct Settings {}
            complex->
                Settings {
                    direct = [PENDING]
                    computed = [1, PENDING + 2]
                }
        ",
        Stage::Type,
    )
    .expect_ok();

    let (direct_expr, direct_value) = root_option_expr_and_value(&resolution, "direct");
    assert_eq!(
        concrete_type(&resolution, direct_expr.type_id),
        TypeId::new(CORE_I64)
    );
    assert_i64_array("direct", direct_value, &[4]);

    let (computed_expr, computed_value) = root_option_expr_and_value(&resolution, "computed");
    assert_eq!(
        concrete_type(&resolution, computed_expr.type_id),
        TypeId::new(CORE_I64)
    );
    assert_i64_array("computed", computed_value, &[1, 6]);
}

#[test]
fn standing_expr_updates_resolved_value_type() {
    let resolution = resolve_single_module(
        "
            let PENDING = SOURCE
            let SOURCE = 4
            nest->
                struct Settings {}
            complex->
                Settings { values = [PENDING] }
        ",
        Stage::Type,
    )
    .expect_ok();

    let (expr, value) = root_option_expr_and_value(&resolution, "values");
    assert_eq!(
        concrete_type(&resolution, expr.type_id),
        TypeId::new(CORE_I64)
    );
    assert_eq!(
        concrete_type(&resolution, value.type_id),
        TypeId::new(CORE_I64),
        "resolving a standing expression must update its cached value type with its expression type"
    );
}
