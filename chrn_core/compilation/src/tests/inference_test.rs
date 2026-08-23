use super::helpers::*;
use crate::parser::ast::ast_concepts::BinaryOp;
use crate::script_compiler::compiler_constants::{
    CORE_BOOL, CORE_CHAR, CORE_F64, CORE_I64, CORE_STR, builtin_ty_to_id,
};
use crate::script_compiler::helpers::core_helpers::{
    CORE_BUILTIN_TYPES_DATASET, core_instantiation_reservations,
};
use crate::script_compiler::helpers::instantiation_symbols::InstantiationSymbolKind;
use crate::semantic::hir::hir_concepts::Type;
use crate::semantic::hir::hir_symbols::SymbolKind;
use crate::semantic::inference::{infer_type_from_binary_op, infer_type_from_val};
use chrn_utils::arena::Arena;
use chrn_utils::id_types::{SymbolId, TypeId};

// -- Helpers --

/// A compiler holding only the implicit `core` module, which is all `infer_type_from_val` needs.
fn core_only_compiler() -> ScriptCompiler {
    ScriptCompiler::init(None, Arena::new())
}

/// Returns the symbol id of the first core function alongside its declared return type.
fn first_core_func(compiler: &ScriptCompiler) -> (SymbolId, TypeId) {
    for symbol in compiler.symbols.iter() {
        if let SymbolKind::Type(type_id) = &symbol.kind {
            if let Type::Func(func_def) = &compiler.types[*type_id].ty {
                return (symbol.sym_id, func_def.ret_type);
            }
        }
    }
    panic!("core should load at least one function");
}

/// Type id of a resolved `let` variable's value, the end product of inference.
fn type_of(compiler: &ScriptCompiler, interner: &Intern, name: &str) -> TypeId {
    let name_id = interner
        .try_search_str(name)
        .unwrap_or_else(|| panic!("Variable '{}' was not interned", name));
    let var_def = compiler
        .variables
        .iter()
        .find(|v| v.name_id == name_id)
        .unwrap_or_else(|| panic!("Variable '{}' not found", name));

    match &var_def.state {
        VariableState::Known(value_id) => compiler.values[*value_id].type_id,
        VariableState::ReservedTypeSlot(_) => {
            panic!("Variable '{}' is still a reserved type slot", name)
        }
    }
}

/// Every op that produces a bool regardless of operand types.
const BOOL_PRODUCING: [BinaryOp; 8] = [
    BinaryOp::Greater,
    BinaryOp::Less,
    BinaryOp::GreaterOrEq,
    BinaryOp::LessOrEq,
    BinaryOp::And,
    BinaryOp::Or,
    BinaryOp::EqTo,
    BinaryOp::NotEq,
];

/// Every op that propagates an operand type rather than producing a fixed one.
const ARITHMETIC: [BinaryOp; 5] = [
    BinaryOp::Add,
    BinaryOp::Sub,
    BinaryOp::Mult,
    BinaryOp::Div,
    BinaryOp::Mod,
];

const BITWISE: [BinaryOp; 5] = [
    BinaryOp::BitOr,
    BinaryOp::BitAnd,
    BinaryOp::BitRightShift,
    BinaryOp::BitLeftShift,
    BinaryOp::BitXor,
];

// -- infer_type_from_val --

#[test]
fn infer_val_maps_scalars_to_core_types() {
    let compiler = core_only_compiler();

    let cases = [
        (Value::I64(7), CORE_I64),
        (Value::F64(7.5), CORE_F64),
        (Value::Bool(true), CORE_BOOL),
        (Value::Char('x'), CORE_CHAR),
        (Value::InternedStr(InternedId::new(0)), CORE_STR),
    ];

    for (val, expected) in cases {
        assert_eq!(
            infer_type_from_val(&compiler, &val),
            Some(TypeId::new(expected)),
            "{:?} should infer core type {}",
            val,
            expected
        );
    }
}

/// The scalar arms are width-agnostic: every integer literal lands on `i64` and every float on
/// `f64`, because `Value` carries no width. Narrowing is not inference's job.
#[test]
fn infer_val_ignores_literal_width() {
    let compiler = core_only_compiler();

    assert_eq!(
        infer_type_from_val(&compiler, &Value::I64(0)),
        infer_type_from_val(&compiler, &Value::I64(i64::MAX))
    );
    assert_eq!(
        infer_type_from_val(&compiler, &Value::F64(0.0)),
        infer_type_from_val(&compiler, &Value::F64(f64::MAX))
    );
}

/// Interning is not consulted: any `InternedId` is `str`, valid or not.
#[test]
fn infer_val_str_does_not_read_the_interner() {
    let compiler = core_only_compiler();

    let out_of_range = InternedId::new(u32::MAX);
    assert_eq!(
        infer_type_from_val(&compiler, &Value::InternedStr(out_of_range)),
        Some(TypeId::new(CORE_STR))
    );
}

#[test]
fn infer_val_unknown_is_none() {
    let compiler = core_only_compiler();

    assert_eq!(infer_type_from_val(&compiler, &Value::Unknown), None);
}

/// An array infers to its *element* type, not a collection type. Arrays have no distinct core
/// type id, so the element type is what callers get.
#[test]
fn infer_val_array_yields_element_type() {
    let compiler = core_only_compiler();

    let ints = Value::Array(vec![Value::I64(1), Value::I64(2)]);
    assert_eq!(
        infer_type_from_val(&compiler, &ints),
        Some(TypeId::new(CORE_I64))
    );

    let strs = Value::Array(vec![Value::InternedStr(InternedId::new(0))]);
    assert_eq!(
        infer_type_from_val(&compiler, &strs),
        Some(TypeId::new(CORE_STR))
    );
}

/// Only the first element is inspected, so a heterogeneous array reports the first element's
/// type instead of rejecting the array. Element agreement is checked elsewhere.
#[test]
fn infer_val_array_only_reads_first_element() {
    let compiler = core_only_compiler();

    let mixed = Value::Array(vec![Value::Bool(true), Value::I64(1), Value::Char('c')]);
    assert_eq!(
        infer_type_from_val(&compiler, &mixed),
        Some(TypeId::new(CORE_BOOL))
    );
}

/// Recursion flattens: nesting depth is discarded and the innermost scalar wins.
#[test]
fn infer_val_nested_array_flattens_to_scalar() {
    let compiler = core_only_compiler();

    let nested = Value::Array(vec![Value::Array(vec![Value::Array(vec![Value::F64(
        1.0,
    )])])]);
    assert_eq!(
        infer_type_from_val(&compiler, &nested),
        Some(TypeId::new(CORE_F64))
    );
}

#[test]
fn infer_val_empty_array_is_none() {
    let compiler = core_only_compiler();

    assert_eq!(infer_type_from_val(&compiler, &Value::Array(vec![])), None);
}

/// An empty array nested inside another array poisons the outer inference, since the recursive
/// call is returned as-is.
#[test]
fn infer_val_nested_empty_array_is_none() {
    let compiler = core_only_compiler();

    let nested_empty = Value::Array(vec![Value::Array(vec![])]);
    assert_eq!(infer_type_from_val(&compiler, &nested_empty), None);
}

/// A function value infers to its return type, not to the function type itself.
#[test]
fn infer_val_func_yields_return_type() {
    let compiler = core_only_compiler();
    let (sym_id, ret_type) = first_core_func(&compiler);

    assert_eq!(
        infer_type_from_val(&compiler, &Value::Func(sym_id)),
        Some(ret_type)
    );
}

/// Tuples exist only to express type constraints and never reach inference as a value.
#[test]
#[should_panic]
fn infer_val_tuple_is_unreachable() {
    let compiler = core_only_compiler();

    infer_type_from_val(&compiler, &Value::Tuple(vec![Value::I64(1)]));
}

/// There are no runtime values at compile time, so a `RuntimeStr` reaching here is a compiler bug.
#[test]
#[should_panic]
fn infer_val_runtime_str_is_unreachable() {
    let compiler = core_only_compiler();

    infer_type_from_val(&compiler, &Value::RuntimeStr(String::from("hi")));
}

// -- infer_type_from_binary_op --

/// With both operand types known, arithmetic adopts the *right* operand's type.
#[test]
fn infer_binary_arithmetic_takes_rhs_when_both_known() {
    let lhs = TypeId::new(100);
    let rhs = TypeId::new(200);

    for op in ARITHMETIC {
        assert_eq!(
            infer_type_from_binary_op(lhs, rhs, false, op, false),
            Some(rhs),
            "{:?} should adopt the rhs type",
            op
        );
    }
}

/// An unknown rhs falls back to the lhs type, which is the only usable one left.
#[test]
fn infer_binary_arithmetic_falls_back_to_lhs_when_rhs_unknown() {
    let lhs = TypeId::new(100);
    let rhs = TypeId::new(200);

    for op in ARITHMETIC {
        assert_eq!(
            infer_type_from_binary_op(lhs, rhs, false, op, true),
            Some(lhs),
            "{:?} should fall back to the lhs type",
            op
        );
    }
}

/// An unknown lhs still yields the rhs type, since rhs is the default branch.
#[test]
fn infer_binary_arithmetic_takes_rhs_when_lhs_unknown() {
    let lhs = TypeId::new(100);
    let rhs = TypeId::new(200);

    for op in ARITHMETIC {
        assert_eq!(
            infer_type_from_binary_op(lhs, rhs, true, op, false),
            Some(rhs),
            "{:?} should adopt the rhs type",
            op
        );
    }
}

/// Nothing to propagate when neither side is known. Callers allocate an `Unknown` type on `None`.
#[test]
fn infer_binary_arithmetic_both_unknown_is_none() {
    let lhs = TypeId::new(100);
    let rhs = TypeId::new(200);

    for op in ARITHMETIC {
        assert_eq!(
            infer_type_from_binary_op(lhs, rhs, true, op, true),
            None,
            "{:?} should not infer a type",
            op
        );
    }
}

/// Comparisons and logical ops produce `bool` no matter what the operands are, including when
/// both are unknown — the result type does not depend on them.
#[test]
fn infer_binary_comparisons_always_produce_bool() {
    let lhs = TypeId::new(100);
    let rhs = TypeId::new(200);
    let expected = Some(TypeId::new(CORE_BOOL));

    for op in BOOL_PRODUCING {
        for (lhs_unknown, rhs_unknown) in
            [(false, false), (true, false), (false, true), (true, true)]
        {
            assert_eq!(
                infer_type_from_binary_op(lhs, rhs, lhs_unknown, op, rhs_unknown),
                expected,
                "{:?} should produce bool (lhs_unknown={}, rhs_unknown={})",
                op,
                lhs_unknown,
                rhs_unknown
            );
        }
    }
}

/// Bitwise ops are pinned to `i64` rather than being endomorphic over the operand type, so a
/// bitwise op on narrower integers still reports `i64`. Flagged `//WARN: Endo` in-source.
#[test]
fn infer_binary_bitwise_always_produces_i64() {
    let lhs = TypeId::new(100);
    let rhs = TypeId::new(200);
    let expected = Some(TypeId::new(CORE_I64));

    for op in BITWISE {
        for (lhs_unknown, rhs_unknown) in
            [(false, false), (true, false), (false, true), (true, true)]
        {
            assert_eq!(
                infer_type_from_binary_op(lhs, rhs, lhs_unknown, op, rhs_unknown),
                expected,
                "{:?} should produce i64 (lhs_unknown={}, rhs_unknown={})",
                op,
                lhs_unknown,
                rhs_unknown
            );
        }
    }
}

/// Only arithmetic consults the unknown flags. Every other op returns a type even when both
/// operands are unknown, so `None` is exclusive to the arithmetic path.
#[test]
fn infer_binary_only_arithmetic_returns_none() {
    let lhs = TypeId::new(100);
    let rhs = TypeId::new(200);

    for op in BOOL_PRODUCING.into_iter().chain(BITWISE) {
        assert!(
            infer_type_from_binary_op(lhs, rhs, true, op, true).is_some(),
            "{:?} should infer a type even with unknown operands",
            op
        );
    }
}

// -- Through the pipeline --

/// The const-folded path: a fully evaluated binary expression infers from the resulting value,
/// not from the operand types.
#[test]
fn inference_through_type_resolution() {
    let (compiler, interner) = compile_and_resolve_single_module(
        "
            let SUM = 1 + 2
            let QUOT = 9 / 3
            let CMP = 1 < 2
            let LOGIC = true == false
            let MASK = 6 | 1
            let FLOAT = 1.5 + 2.5
        ",
    );

    let cases = [
        ("SUM", CORE_I64),
        ("QUOT", CORE_I64),
        ("CMP", CORE_BOOL),
        ("LOGIC", CORE_BOOL),
        ("MASK", CORE_I64),
        ("FLOAT", CORE_F64),
    ];

    for (name, expected) in cases {
        assert_eq!(
            type_of(&compiler, &interner, name),
            TypeId::new(expected),
            "'{}' should infer core type {}",
            name,
            expected
        );
    }
}

/// A chain of const dependencies keeps the inferred type through every step, so inference does
/// not decay across the resolution order.
#[test]
fn inference_survives_const_dependency_chain() {
    let (compiler, interner) = compile_and_resolve_single_module(
        "
            let A = B + 1
            let B = C * 2
            let C = 3
        ",
    );

    for name in ["A", "B", "C"] {
        assert_eq!(
            type_of(&compiler, &interner, name),
            TypeId::new(CORE_I64),
            "'{}' should infer i64",
            name
        );
    }
}

// -- Intrinsic namespace constants --

/// Every intrinsic constant, as `(source path, the `TypeId` it must have, its value)`.
fn intrinsic_constants() -> Vec<(String, TypeId, Value)> {
    let interner = Intern::init();
    let mut constants = Vec::new();

    for (interned, builtin_ty, ns) in &CORE_BUILTIN_TYPES_DATASET {
        let ty_name = interner.search_idx(*interned as usize);
        let type_id = TypeId::new(builtin_ty_to_id(builtin_ty.kind()));

        for base in ns.iter() {
            let InstantiationSymbolKind::Variable(var) = &base.kind else {
                panic!("`{ty_name}` holds a non-variable intrinsic entry");
            };

            let bound = interner.search_idx(base.name_id.id as usize);
            constants.push((format!("{ty_name}::{bound}"), type_id, var.val.to_val()));
        }
    }

    constants
}

/// A constant reached through a built-in's namespace keeps that built-in's type, not the type
/// its `Value` payload would infer. `u8::MAX` is a `u8` holding an `I64`, and `f16::MAX` an
/// `f16` holding an `F64`, so inferring from the payload would collapse both onto `i64`/`f64`.
#[test]
fn infer_intrinsic_namespace_constant_test() {
    for (path, type_id, expected) in intrinsic_constants() {
        let text = format!("let CONSTANT = {path}");
        let (compiler, interner) = resolve_single_module(&text, Stage::Constraint)
            .expect_ok()
            .into_state();

        let name_id = interner
            .try_search_str("CONSTANT")
            .expect("`CONSTANT` should be interned");
        let var_def = compiler
            .variables
            .iter()
            .find(|var| var.name_id == name_id)
            .expect("`CONSTANT` should be declared");

        let VariableState::Known(val_id) = var_def.state else {
            panic!("`{path}` did not resolve to a known value");
        };

        let val_info = &compiler.values[val_id];

        assert_eq!(val_info.type_id, type_id, "type of `{path}`");

        let found = val_info
            .const_val
            .as_ref()
            .unwrap_or_else(|| panic!("`{path}` has no constant value"));

        assert!(
            values_eq(found, &expected),
            "`{path}` is {found:?}, expected {expected:?}"
        );
    }
}

/// A `let` bound to an intrinsic constant aliases that constant's `ValueId` rather than copying
/// it, so every user expression shares the slot `i8::MAX` lives in. Nothing a user writes may
/// write back through that slot, or one script would rewrite a bound for the rest of the run.
#[test]
fn intrinsic_namespace_constants_are_not_rewritten_test() {
    let generated = core_instantiation_reservations().variables;
    let baseline = core_only_compiler();

    let sources = [
        "let CONSTANT = i8::MAX",
        "let CONSTANT = -i8::MAX",
        "let CONSTANT = i8::MAX + 1",
        "let FIRST = SECOND\nlet SECOND = u32::MAX",
        "let CONSTANT = f64::MIN\nvar->\n    field: i8 [Range(i8::MIN, i8::MAX)]",
    ];

    for text in sources {
        let (compiler, _) = resolve_single_module(text, Stage::Constraint)
            .expect_ok()
            .into_state();

        for idx in 0..generated {
            let val_id = ValueId::new(idx as u32);
            let found = &compiler.values[val_id];
            let expected = &baseline.values[val_id];

            assert_eq!(
                found.type_id, expected.type_id,
                "{text:?} changed the type of intrinsic {val_id:?}"
            );

            match (&found.const_val, &expected.const_val) {
                (Some(found_val), Some(expected_val)) => assert!(
                    values_eq(found_val, expected_val),
                    "{text:?} changed intrinsic {val_id:?} to {found_val:?}, was {expected_val:?}"
                ),
                (found_opt, expected_opt) => panic!(
                    "{text:?} changed intrinsic {val_id:?} from {expected_opt:?} to {found_opt:?}"
                ),
            }

            // The built-in each constant is typed as must still be that built-in, since the
            // repair path for forward references writes `Deferred` through a value's type slot
            assert!(
                matches!(compiler.types[found.type_id].ty, Type::BuiltinTypeInfo(_)),
                "{text:?} overwrote the builtin at {:?}",
                found.type_id
            );
        }
    }
}
