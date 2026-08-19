use super::helpers::*;

#[test]
fn variable_declaration_test() {
    // let CONSTANT = 4
    let text = "
            let CONSTANT = 4
            ";

    let (compiler, _) = compile_and_resolve_single_module(text);

    assert_eq!(compiler.values.len(), 1);
    let last_val = &compiler.values[ValueId::new(0)];
    match &last_val.const_val {
        Some(Value::I64(4)) => (),
        _ => panic!("Value mismatch, expected I64(4)"),
    };

    // let CONSTANT = "Hallo"
    let text = "
            let CONSTANT = \"Hallo\"
        ";

    let (compiler, interner) = compile_and_resolve_single_module(text);

    assert_eq!(compiler.values.len(), 1);
    let last_val = &compiler.values[ValueId::new(0)];
    match &last_val.const_val {
        Some(Value::InternedStr(id)) => {
            assert_eq!("Hallo", interner.search(*id));
        }
        _ => panic!("Value mismatch, expected InternedStr(\"Hallo\")"),
    };

    // let CONSTANT = 0e-5
    let text = "
            let CONSTANT = 0e-5
        ";

    let (compiler, _) = compile_and_resolve_single_module(text);

    assert_eq!(compiler.values.len(), 1);
    let last_val = &compiler.values[ValueId::new(0)];
    match &last_val.const_val {
        Some(Value::F64(v)) if *v == 0e-5 => (),
        _ => panic!("Value mismatch, expected F64(0e-5)"),
    };

    // let CONSTANT = true
    let text = "
            let CONSTANT = true
        ";

    let (compiler, _) = compile_and_resolve_single_module(text);

    assert_eq!(compiler.values.len(), 1);
    let last_val = &compiler.values[ValueId::new(0)];
    match &last_val.const_val {
        Some(Value::Bool(true)) => (),
        _ => panic!("Value mismatch, expected Bool(true)"),
    };

    // let CONSTANT = false
    let text = "
            let CONSTANT = false
        ";

    let (compiler, _) = compile_and_resolve_single_module(text);

    assert_eq!(compiler.values.len(), 1);
    let last_val = &compiler.values[ValueId::new(0)];
    match &last_val.const_val {
        Some(Value::Bool(false)) => (),
        _ => panic!("Value mismatch, expected Bool(false)"),
    };

    // let character = 'c'
    let text = "
            let character = 'c'
        ";

    let (compiler, _) = compile_and_resolve_single_module(text);

    assert_eq!(compiler.values.len(), 1);
    let last_val = &compiler.values[ValueId::new(0)];
    match &last_val.const_val {
        Some(Value::Char('c')) => (),
        _ => panic!("Value mismatch, expected Char('c')"),
    };
}

#[test]
fn type_resolver_values_test() {
    let text = "
            let CONSTANT_INT = 4
            let CONSTANT_STR = \"Hallo\"
            let CONSTANT_FLOAT = 0e-5
            let CONSTANT_TRUE = true
            let CONSTANT_FALSE = false
            let CONSTANT_CHAR = 'c'
        ";

    let (compiler, interner) = compile_and_resolve_single_module(text);

    let find_val = |name: &str| -> &Value {
        let name_id = interner.try_search_str(name).unwrap();
        let var_def = compiler
            .variables
            .iter()
            .find(|v| v.name_id == name_id)
            .expect("Variable '{name}' not found");
        match &var_def.state {
            VariableState::Known(value_id) => compiler.values[*value_id]
                .const_val
                .as_ref()
                .expect("Variable '{name}' has no const_val"),
            VariableState::ReservedTypeSlot(_) => {
                panic!("Variable '{name}' is not yet resolved")
            }
        }
    };

    assert_eq!(compiler.values.len(), 6);
    assert!(matches!(find_val("CONSTANT_INT"), Value::I64(4)));
    assert!(
        matches!(find_val("CONSTANT_STR"), Value::InternedStr(id) if interner.search(*id) == "Hallo")
    );
    assert!(matches!(find_val("CONSTANT_FLOAT"), Value::F64(v) if *v == 0e-5));
    assert!(matches!(find_val("CONSTANT_TRUE"), Value::Bool(true)));
    assert!(matches!(find_val("CONSTANT_FALSE"), Value::Bool(false)));
    assert!(matches!(find_val("CONSTANT_CHAR"), Value::Char('c')));
}

#[test]
fn all_operators_test() {
    let eval = |text: &str| -> Value {
        resolve_single_module(text, Stage::Constraint)
            .expect_ok()
            .value_of("X")
    };

    // -- Unary: ! (Not) --
    assert!(matches!(eval("let X = !true"), Value::Bool(false)));
    assert!(matches!(eval("let X = !false"), Value::Bool(true)));
    // -- Unary: - (Negate) --
    assert!(matches!(eval("let X = -5"), Value::I64(-5)));
    assert!(matches!(eval("let X = -3.14"), Value::F64(v) if v == -3.14));
    // -- Unary: ~ (BitNot) --
    assert!(matches!(eval("let X = ~5"), Value::I64(x) if x == !5));

    // -- Binary: + --
    assert!(matches!(eval("let X = 10 + 20"), Value::I64(30)));
    assert!(matches!(eval("let X = 1.5 + 2.5"), Value::F64(v) if v == 4.0));
    // -- Binary: - --
    assert!(matches!(eval("let X = 10 - 3"), Value::I64(7)));
    assert!(matches!(eval("let X = 5.5 - 1.5"), Value::F64(v) if v == 4.0));
    // -- Binary: * --
    assert!(matches!(eval("let X = 3 * 7"), Value::I64(21)));
    assert!(matches!(eval("let X = 2.5 * 4.0"), Value::F64(v) if v == 10.0));
    // -- Binary: / --
    assert!(matches!(eval("let X = 10 / 3"), Value::I64(3)));
    assert!(matches!(eval("let X = 10.0 / 4.0"), Value::F64(v) if v == 2.5));
    // -- Binary: % --
    assert!(matches!(eval("let X = 10 % 3"), Value::I64(1)));

    // -- Binary: > --
    assert!(matches!(eval("let X = 5 > 3"), Value::Bool(true)));
    assert!(matches!(eval("let X = 3 > 5"), Value::Bool(false)));
    // -- Binary: < --
    assert!(matches!(eval("let X = 5 < 3"), Value::Bool(false)));
    assert!(matches!(eval("let X = 3 < 5"), Value::Bool(true)));
    // -- Binary: >= --
    assert!(matches!(eval("let X = 5 >= 3"), Value::Bool(true)));
    assert!(matches!(eval("let X = 5 >= 5"), Value::Bool(true)));
    assert!(matches!(eval("let X = 3 >= 5"), Value::Bool(false)));
    // -- Binary: <= --
    assert!(matches!(eval("let X = 3 <= 5"), Value::Bool(true)));
    assert!(matches!(eval("let X = 5 <= 5"), Value::Bool(true)));
    assert!(matches!(eval("let X = 5 <= 3"), Value::Bool(false)));
    // -- Binary: == --
    assert!(matches!(eval("let X = 5 == 5"), Value::Bool(true)));
    assert!(matches!(eval("let X = 5 == 3"), Value::Bool(false)));
    // -- Binary: != --
    assert!(matches!(eval("let X = 5 != 3"), Value::Bool(true)));
    assert!(matches!(eval("let X = 5 != 5"), Value::Bool(false)));

    // -- Binary: && --
    assert!(matches!(eval("let X = true && true"), Value::Bool(true)));
    assert!(matches!(eval("let X = true && false"), Value::Bool(false)));
    // -- Binary: || --
    assert!(matches!(eval("let X = true || false"), Value::Bool(true)));
    assert!(matches!(eval("let X = false || false"), Value::Bool(false)));

    // -- Binary: | (BitOr) --
    assert!(matches!(eval("let X = 5 | 3"), Value::I64(7)));
    // -- Binary: & (BitAnd) --
    assert!(matches!(eval("let X = 5 & 3"), Value::I64(1)));
    // -- Binary: ^ (BitXor) --
    assert!(matches!(eval("let X = 5 ^ 3"), Value::I64(6)));
    // -- Binary: << (BitLeftShift) --
    assert!(matches!(eval("let X = 1 << 2"), Value::I64(4)));
    // -- Binary: >> (BitRightShift) --
    assert!(matches!(eval("let X = 8 >> 1"), Value::I64(4)));

    // -- String comparison (!= only) --
    assert!(matches!(
        eval("let X = \"hello\" != \"world\""),
        Value::Bool(true)
    ));
    assert!(matches!(
        eval("let X = \"hello\" != \"hello\""),
        Value::Bool(false)
    ));

    // -- Char comparison --
    assert!(matches!(eval("let X = 'b' > 'a'"), Value::Bool(true)));
    assert!(matches!(eval("let X = 'a' == 'a'"), Value::Bool(true)));
    assert!(matches!(eval("let X = 'a' != 'b'"), Value::Bool(true)));
    assert!(matches!(eval("let X = 'a' < 'b'"), Value::Bool(true)));
    assert!(matches!(eval("let X = 'a' <= 'b'"), Value::Bool(true)));
    assert!(matches!(eval("let X = 'b' >= 'a'"), Value::Bool(true)));
    assert!(matches!(eval("let X = 'a' <= 'a'"), Value::Bool(true)));
    assert!(matches!(eval("let X = 'b' >= 'b'"), Value::Bool(true)));

    // -- Bool comparison (==, !=) --
    assert!(matches!(eval("let X = true == true"), Value::Bool(true)));
    assert!(matches!(eval("let X = true == false"), Value::Bool(false)));
    assert!(matches!(eval("let X = true != false"), Value::Bool(true)));

    // -- Float comparison --
    assert!(matches!(eval("let X = 3.14 > 2.0"), Value::Bool(true)));
    assert!(matches!(eval("let X = 3.14 == 3.14"), Value::Bool(true)));
    assert!(matches!(eval("let X = 3.14 != 2.0"), Value::Bool(true)));

    // -- Float mod --
    assert!(matches!(eval("let X = 5.5 % 2.0"), Value::F64(v) if v == 1.5));
}

#[test]
fn const_dependency_resolution_test() {
    // Ok buddy
    let approx_eq = |a: f64, b: f64| (a - b).abs() < 1e-9;

    // 1) Reverse-ordered linear chain: each variable depends on the previous one, and the
    //    literal is declared last. This exercises the pending-expression propagation loop.
    let (compiler, interner) = compile_and_resolve_single_module(
        "
            let A = E + 2
            let B = A * 3
            let C = B - 1
            let D = C / 2
            let E = 4
        ",
    );
    assert!(matches!(value_of(&compiler, &interner, "D"), Value::I64(8)));

    // 2) Diamond dependency: one base value feeds two branches that are later combined.
    let (compiler, interner) = compile_and_resolve_single_module(
        "
            let BASE = 2
            let LEFT = BASE * 3
            let RIGHT = BASE + 5
            let TOP = LEFT + RIGHT
        ",
    );
    assert!(matches!(
        value_of(&compiler, &interner, "TOP"),
        Value::I64(13)
    ));

    // 3) Expression declared before its dependencies, referencing multiple pending variables.
    let (compiler, interner) = compile_and_resolve_single_module(
        "
            let Z = (X + Y) * (Y - W)
            let W = 2
            let X = W + 3
            let Y = X * W
        ",
    );
    assert!(matches!(
        value_of(&compiler, &interner, "Z"),
        Value::I64(120)
    ));

    // 4) Long chain of pure references.
    let (compiler, interner) = compile_and_resolve_single_module(
        "
            let N1 = 7
            let N2 = N1
            let N3 = N2
            let N4 = N3
            let N5 = N4 + N3 * 2
        ",
    );
    assert!(matches!(
        value_of(&compiler, &interner, "N5"),
        Value::I64(21)
    ));

    // What is thresh 😭
    // 5) Boolean values derived from numeric comparisons.
    let (compiler, interner) = compile_and_resolve_single_module(
        "
            let THRESH = 5
            let VAL = 10
            let IS_BIG = VAL > THRESH
            let RESULT = IS_BIG || false
        ",
    );
    assert!(matches!(
        value_of(&compiler, &interner, "RESULT"),
        Value::Bool(true)
    ));

    // 6) Floating-point dependency chain.
    let (compiler, interner) = compile_and_resolve_single_module(
        "
            let PI = 3.14
            let R = 2.0
            let AREA = PI * R * R
        ",
    );
    match value_of(&compiler, &interner, "AREA") {
        Value::F64(v) => assert!(approx_eq(v, 12.56), "AREA was {}", v),
        other => panic!("Expected F64 for AREA, got {:?}", other),
    }

    // 7) Unary operator propagation through a dependency.
    let (compiler, interner) = compile_and_resolve_single_module(
        "
            let NEG = -5
            let POS = -NEG + 1
        ",
    );
    assert!(matches!(
        value_of(&compiler, &interner, "POS"),
        Value::I64(6)
    ));

    // 8) Mixed int/bool independent chains in the same module.
    let (compiler, interner) = compile_and_resolve_single_module(
        "
            let A = 3
            let B = 4
            let C = A > B
            let D = !C
            let E = (A + B) * 2
            let F = E > 10
        ",
    );
    assert!(matches!(
        value_of(&compiler, &interner, "D"),
        Value::Bool(true)
    ));
    assert!(matches!(
        value_of(&compiler, &interner, "F"),
        Value::Bool(true)
    ));
}

#[test]
fn const_dependency_circular_test() {
    let assert_any_var_unknown = |(result, compiler, interner): (
        Result<(), Vec<SourceDiagnostic>>,
        ScriptCompiler,
        Intern,
    ),
                                  names: &[&str]| {
        assert!(result.is_err(), "Circular dependency should be rejected");
        let any_unknown = names.iter().any(|name| {
            let name_id = interner
                .try_search_str(name)
                .unwrap_or_else(|| panic!("Variable '{name}' was not interned"));
            let var_def = compiler
                .variables
                .iter()
                .find(|v| v.name_id == name_id)
                .unwrap_or_else(|| panic!("Variable '{name}' not found"));
            matches!(var_def.state, VariableState::ReservedTypeSlot(_))
        });
        assert!(
            any_unknown,
            "At least one variable in the cycle should remain unresolved (ReservedTypeSlot), but all were Known: {:?}",
            names
                .iter()
                .map(|name| {
                    let name_id = interner.try_search_str(name).unwrap();
                    let var_def = compiler
                        .variables
                        .iter()
                        .find(|v| v.name_id == name_id)
                        .unwrap();
                    (name, &var_def.state)
                })
                .collect::<Vec<_>>()
        );
    };

    // Linear dependency cycle should be rejected
    assert_any_var_unknown(
        type_resolve_single_module_keep_state("let x = y\nlet y = x"),
        &["x", "y"],
    );

    // Direct self reference.
    assert_any_var_unknown(type_resolve_single_module_keep_state("let X = X"), &["X"]);

    // Three-variable cycle.
    assert_any_var_unknown(
        type_resolve_single_module_keep_state("let A = B + 1\nlet B = C * 2\nlet C = A"),
        &["A", "B", "C"],
    );

    // Long indirect cycle.
    assert_any_var_unknown(
        type_resolve_single_module_keep_state(
            "
                let A = B
                let B = C
                let C = D
                let D = E
                let E = A
            ",
        ),
        &["A", "B", "C", "D", "E"],
    );

    // Cycle hidden inside a larger expression.
    assert_any_var_unknown(
        type_resolve_single_module_keep_state("let X = (Y + 2) * 3\nlet Y = X - 1"),
        &["X", "Y"],
    );

    // Multiple independent cycles in the same module.
    assert_any_var_unknown(
        type_resolve_single_module_keep_state(
            "
                let A = B
                let B = A
                let C = D + 1
                let D = C
            ",
        ),
        &["A", "B", "C", "D"],
    );

    // A chain that leads into a cycle.
    assert_any_var_unknown(
        type_resolve_single_module_keep_state(
            "
                let A = B + 1
                let B = C
                let C = B
            ",
        ),
        &["A", "B", "C"],
    );
}
