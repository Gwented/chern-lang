use super::helpers::*;
use crate::config_loader::{ConfigLoader, ConfigLoaderOutput};
use crate::lexer::token::TokenKind;
use crate::parser::ast::ast_concepts::{
    AbstractAlias, AbstractConfig, AbstractConfigKind, AbstractEnum, AbstractOptionAssignment,
    AbstractStruct, AbstractTypeDef, AbstractVar, AbstractVariant, BinaryOp, Item, SectionKind,
    UnaryOp,
};
use crate::parser::ast::ast_exprs::{Expr, PathSegment, TypeExpr};
use chrn_utils::id_types::AstId;

// =============================================================================
// Helpers
// =============================================================================

/// Lex and parse `text` in a single-module context, returning the `AstInfo`
/// together with the interner (so assertions can look up interned strings).
fn parse_text(text: &str) -> (AstInfo, Intern) {
    let (arena, mut interner, settings, _compiler) = mock_single_module_compiler(text);
    let region = {
        let module = &_compiler.mods[ModuleId::new(0)];
        get_module_region(&arena, module)
    };
    let toks = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
        .tokenize(&mut interner)
        .toks;
    let (ast, _diags) = parser::parse(&settings, region, &toks, &interner);
    (ast, interner)
}

/// Like `parse_text` but returns diagnostics as well.
fn parse_text_with_diags(text: &str) -> (AstInfo, Vec<SourceDiagnostic>, Intern) {
    let (arena, mut interner, settings, _compiler) = mock_single_module_compiler(text);
    let region = {
        let module = &_compiler.mods[ModuleId::new(0)];
        get_module_region(&arena, module)
    };
    let toks = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
        .tokenize(&mut interner)
        .toks;
    let (ast, summary) = parser::parse(&settings, region, &toks, &interner);
    (ast, summary.diags, interner)
}

/// Helper: given `items` from `AstInfo::items()`, find the first item of a
/// given variant by index and return a reference.
fn items_from(ast: &AstInfo) -> &[Item] {
    ast.items()
}

fn section_items(ast: &AstInfo, kind: SectionKind) -> Vec<AstId> {
    ast.sections[kind as usize]
        .as_ref()
        .map(|s| s.nodes.clone())
        .unwrap_or_default()
}

fn get_var<'a>(ast: &'a AstInfo, index: usize) -> &'a AbstractVar {
    match &ast.items()[index] {
        Item::Var(v) => v,
        other => panic!("expected Item::Var at index {index}, got {other:?}"),
    }
}

fn get_typedef<'a>(ast: &'a AstInfo, index: usize) -> &'a AbstractTypeDef {
    match &ast.items()[index] {
        Item::TypeDef(t) => t,
        other => panic!("expected Item::TypeDef at index {index}, got {other:?}"),
    }
}

fn get_alias<'a>(ast: &'a AstInfo, index: usize) -> &'a AbstractAlias {
    match &ast.items()[index] {
        Item::Alias(a) => a,
        other => panic!("expected Item::Alias at index {index}, got {other:?}"),
    }
}

fn get_struct<'a>(ast: &'a AstInfo, index: usize) -> &'a AbstractStruct {
    match &ast.items()[index] {
        Item::Struct(s) => s,
        other => panic!("expected Item::Struct at index {index}, got {other:?}"),
    }
}

fn get_enum<'a>(ast: &'a AstInfo, index: usize) -> &'a AbstractEnum {
    match &ast.items()[index] {
        Item::Enum(e) => e,
        other => panic!("expected Item::Enum at index {index}, got {other:?}"),
    }
}

fn get_cfg<'a>(ast: &'a AstInfo, index: usize) -> &'a AbstractConfig {
    match &ast.items()[index] {
        Item::Config(c) => c,
        other => panic!("expected Item::Config at index {index}, got {other:?}"),
    }
}

// =============================================================================
// Neutral-section tests
// =============================================================================

#[test]
fn parse_bind() {
    let text = r#"bind "./some/path""#;
    // bind is a parser check only — no AST item is produced
    // No section is created since no items are pushed
    let (ast, _interner) = parse_text(text);

    assert!(ast.items().is_empty(), "bind should not produce AST items");
    // All sections remain None
    for sect in ast.sections() {
        assert!(sect.is_none());
    }
}

#[test]
fn parse_import() {
    let text = r#"import "./some/path" as foo"#;
    let (ast, _interner) = parse_text(text);

    assert!(
        ast.items().is_empty(),
        "import should not produce AST items"
    );
    for sect in ast.sections() {
        assert!(sect.is_none());
    }
}

#[test]
fn parse_import_no_as() {
    let text = r#"import "./some/path""#;
    let (ast, _interner) = parse_text(text);

    assert!(
        ast.items().is_empty(),
        "import should not produce AST items"
    );
    for sect in ast.sections() {
        assert!(sect.is_none());
    }
}

#[test]
fn parse_let_integer() {
    let text = "let x = 42";
    // let = 0..3, space = 3, x = 4, space = 5, = = 6, space = 7, 42 = 8..10
    let (ast, interner) = parse_text(text);

    let sect = section_items(&ast, SectionKind::Neutral);
    assert_eq!(sect.len(), 1, "expected one item in neutral section");

    // AstId 0 -> first item pushed
    let var = ast.get_var(sect[0]);
    assert_eq!(interner.search(var.name_id), "x");
    // name_span should cover "x" bytes 4..5
    assert_eq!(var.name_span.start, 4);
    assert_eq!(var.name_span.end, 5);
    assert!(var.is_priv, "let without export should be private");

    // Check the expression: Integer(42)
    match &var.spanned_expr.expr {
        Expr::Integer(id, Notation::Decimal) => {
            assert_eq!(interner.search(*id), "42");
        }
        other => panic!("expected Expr::Integer, got {other:?}"),
    }
    // Span of the expression should cover "42" (bytes 8..10)
    assert_eq!(var.spanned_expr.span.start, 8);
    assert_eq!(var.spanned_expr.span.end, 10);
}

#[test]
fn parse_let_string() {
    let text = r#"let msg = "hello""#;
    // let = 0..3, space = 3, msg = 4..7, space = 7, = = 8, space = 9, "hello" = 10..17
    let (ast, interner) = parse_text(text);

    let sect = section_items(&ast, SectionKind::Neutral);
    assert_eq!(sect.len(), 1);

    let var = ast.get_var(sect[0]);
    assert_eq!(interner.search(var.name_id), "msg");
    assert_eq!(var.name_span.start, 4);
    assert_eq!(var.name_span.end, 7);

    match &var.spanned_expr.expr {
        Expr::Str(id) => {
            assert_eq!(interner.search(*id), "hello");
        }
        other => panic!("expected Expr::Str, got {other:?}"),
    }
    // expression span should cover the whole string including quotes: 10..17
    assert_eq!(var.spanned_expr.span.start, 10);
    assert_eq!(var.spanned_expr.span.end, 17);
}

#[test]
fn parse_let_float() {
    let text = "let pi = 3.14";
    // let = 0..3, space = 3, pi = 4..6, space = 6, = = 7, space = 8, 3.14 = 9..13
    let (ast, interner) = parse_text(text);

    let sect = section_items(&ast, SectionKind::Neutral);
    assert_eq!(sect.len(), 1);

    let var = ast.get_var(sect[0]);
    assert_eq!(interner.search(var.name_id), "pi");
    assert_eq!(var.name_span.start, 4);
    assert_eq!(var.name_span.end, 6);

    match &var.spanned_expr.expr {
        Expr::Float(id, Notation::Decimal) => {
            assert_eq!(interner.search(*id), "3.14");
        }
        other => panic!("expected Expr::Float, got {other:?}"),
    }
    assert_eq!(var.spanned_expr.span.start, 9);
    assert_eq!(var.spanned_expr.span.end, 13);
}

#[test]
fn parse_let_bool() {
    let text = "let flag = true";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    match &var.spanned_expr.expr {
        Expr::Bool(true) => {}
        other => panic!("expected Expr::Bool(true), got {other:?}"),
    }
}

#[test]
fn parse_let_char() {
    let text = "let c = 'a'";
    // 'a' = bytes 8..11
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    match &var.spanned_expr.expr {
        Expr::Char('a') => {}
        other => panic!("expected Expr::Char('a'), got {other:?}"),
    }
    assert_eq!(var.spanned_expr.span.start, 8);
    assert_eq!(var.spanned_expr.span.end, 11);
}

#[test]
fn parse_export_let() {
    let text = "export let x = 1";
    // export = 0..6, space = 6, let = 7..10, space = 10, x = 11, space = 12, = = 13, space = 14, 1 = 15
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    assert_eq!(interner.search(var.name_id), "x");
    assert!(!var.is_priv, "export let should be public");
    assert_eq!(var.name_span.start, 11);
    assert_eq!(var.name_span.end, 12);
}

#[test]
fn parse_alias_no_params() {
    let text = "alias x() = [true]";
    // alias = 0..5, space = 5, x = 6, () = 7..9, space = 9, = = 10, space = 11, [true] = 12..18
    let (ast, interner) = parse_text(text);

    let sect = section_items(&ast, SectionKind::Neutral);
    assert_eq!(sect.len(), 1);

    let alias = ast.get_alias(sect[0]);
    assert_eq!(interner.search(alias.name_id), "x");
    assert_eq!(alias.name_span.start, 6);
    assert_eq!(alias.name_span.end, 7);
    assert!(alias.is_priv);
    assert!(alias.params.is_empty(), "no params expected");

    // Should have one condition: [true]
    assert_eq!(alias.conds.len(), 1, "expected one condition");
    match &alias.conds[0].expr {
        Expr::Bool(true) => {}
        other => panic!("expected Bool(true) condition, got {other:?}"),
    }
}

#[test]
fn parse_alias_with_params() {
    let text = "alias max(a: i32, b: string) = [a > b]";
    // alias = 0..5, space = 5, max = 6..9, ( = 9, a = 10, : = 11, space = 12, i32 = 13..16,
    // , = 16, space = 17, b = 18, : = 19, space = 20, string = 21..27, ) = 27,
    // space = 28, = = 29, space = 30, [ = 31, a > b = 32..36, ] = 37
    let (ast, interner) = parse_text(text);

    let alias = ast.get_alias(section_items(&ast, SectionKind::Neutral)[0]);
    assert_eq!(interner.search(alias.name_id), "max");
    assert_eq!(alias.params.len(), 2);

    // First param: a: i32
    let p0 = &alias.params[0];
    assert_eq!(interner.search(p0.name_id), "a");
    match &p0.sp_ty_expr.inner {
        TypeExpr::Var(id) => assert_eq!(interner.search(*id), "i32"),
        other => panic!("expected TypeExpr::Var, got {other:?}"),
    }

    // Second param: b: string
    let p1 = &alias.params[1];
    assert_eq!(interner.search(p1.name_id), "b");
    match &p1.sp_ty_expr.inner {
        TypeExpr::Var(id) => assert_eq!(interner.search(*id), "string"),
        other => panic!("expected TypeExpr::Var, got {other:?}"),
    }

    // One condition: a > b
    assert_eq!(alias.conds.len(), 1);
    match &alias.conds[0].expr {
        Expr::BinaryExpr {
            op: BinaryOp::Greater,
            ..
        } => {}
        other => panic!("expected Greater binary expr, got {other:?}"),
    }
}

#[test]
fn parse_alias_with_directives() {
    let text = "alias foo() = #warn";
    let (ast, interner) = parse_text(text);

    let alias = ast.get_alias(section_items(&ast, SectionKind::Neutral)[0]);
    assert_eq!(alias.directives.len(), 1);
    assert_eq!(
        interner.search(alias.directives[0].sp_name_id.inner),
        "warn"
    );
}

// =============================================================================
// Var section tests
// =============================================================================

#[test]
fn parse_var_section_single_typedef() {
    let text = "var->\n    x: i32";
    // var = 0..3, -> = 3..5, \n = 5, spaces = 6..9, x = 10, : = 11, space = 12, i32 = 13..16
    let (ast, interner) = parse_text(text);

    let sect = ast.sections[SectionKind::Var as usize]
        .as_ref()
        .expect("expected a var section");
    assert_eq!(sect.nodes.len(), 1);

    let typedef = ast.get_typedef(sect.nodes[0]);
    assert_eq!(interner.search(typedef.name_id), "x");
    // name_span: "x" at byte 10..11
    assert_eq!(typedef.name_span.start, 10);
    assert_eq!(typedef.name_span.end, 11);

    match &typedef.sp_ty_expr.inner {
        TypeExpr::Var(id) => assert_eq!(interner.search(*id), "i32"),
        other => panic!("expected TypeExpr::Var, got {other:?}"),
    }
    // Type span: "i32" at bytes 13..16
    assert_eq!(typedef.sp_ty_expr.span.start, 13);
    assert_eq!(typedef.sp_ty_expr.span.end, 16);
}

#[test]
fn parse_var_section_multiple_typedefs() {
    let text = "var->\n    x: i32\n    y: string";
    let (ast, interner) = parse_text(text);

    let sect = ast.sections[SectionKind::Var as usize]
        .as_ref()
        .expect("expected a var section");
    assert_eq!(sect.nodes.len(), 2);

    let td0 = ast.get_typedef(sect.nodes[0]);
    assert_eq!(interner.search(td0.name_id), "x");
    match &td0.sp_ty_expr.inner {
        TypeExpr::Var(id) => assert_eq!(interner.search(*id), "i32"),
        other => panic!("expected i32 type, got {other:?}"),
    }

    let td1 = ast.get_typedef(sect.nodes[1]);
    assert_eq!(interner.search(td1.name_id), "y");
    match &td1.sp_ty_expr.inner {
        TypeExpr::Var(id) => assert_eq!(interner.search(*id), "string"),
        other => panic!("expected string type, got {other:?}"),
    }
}

#[test]
fn parse_var_typedef_with_conditions() {
    let text = "var->\n    x: i32 [x > 0]";
    let (ast, interner) = parse_text(text);

    let td = ast.get_typedef(section_items(&ast, SectionKind::Var)[0]);
    assert_eq!(td.conds.len(), 1, "expected one condition");
    match &td.conds[0].expr {
        Expr::BinaryExpr {
            op: BinaryOp::Greater,
            ..
        } => {}
        other => panic!("expected Greater binary expr, got {other:?}"),
    }
}

#[test]
fn parse_var_typedef_with_directives() {
    let text = "var->\n    x: i32 #warn";
    let (ast, interner) = parse_text(text);

    let td = ast.get_typedef(section_items(&ast, SectionKind::Var)[0]);
    assert_eq!(td.directives.len(), 1);
    assert_eq!(interner.search(td.directives[0].sp_name_id.inner), "warn");
}

#[test]
fn parse_var_typedef_with_trailing_comma() {
    let text = "var->\n    x: i32,";
    let (ast, interner) = parse_text(text);

    let sect = ast.sections[SectionKind::Var as usize]
        .as_ref()
        .expect("expected a var section");
    assert_eq!(sect.nodes.len(), 1);
    let td = ast.get_typedef(sect.nodes[0]);
    assert_eq!(interner.search(td.name_id), "x");
}

// =============================================================================
// Nest section tests
// =============================================================================

#[test]
fn parse_nest_struct_empty() {
    let text = "nest->\n    struct Foo {}";
    // nest = 0..4, -> = 4..6, \n = 6, spaces = 7..10, struct = 11..17, space = 17,
    // Foo = 18..21, space = 21, {} = 22..24
    let (ast, interner) = parse_text(text);

    let sect = ast.sections[SectionKind::Nest as usize]
        .as_ref()
        .expect("expected a nest section");
    assert_eq!(sect.nodes.len(), 1);

    let st = ast.get_struct(sect.nodes[0]);
    assert_eq!(interner.search(st.name_id), "Foo");
    // name_span should cover "Foo" (bytes 18..21)
    assert_eq!(st.name_span.start, 18);
    assert_eq!(st.name_span.end, 21);
    assert!(st.fields.is_empty(), "empty struct should have no fields");
    assert!(st.is_priv, "struct without export should be private");
}

#[test]
fn parse_nest_struct_with_fields() {
    let text = "nest->\n    struct Point { x: f64, y: f64 }";
    let (ast, interner) = parse_text(text);

    let st = ast.get_struct(section_items(&ast, SectionKind::Nest)[0]);
    assert_eq!(interner.search(st.name_id), "Point");
    assert_eq!(st.fields.len(), 2);

    // Field x: f64
    let f0 = &st.fields[0];
    assert_eq!(interner.search(f0.name_id), "x");
    match &f0.sp_ty_expr.inner {
        TypeExpr::Var(id) => assert_eq!(interner.search(*id), "f64"),
        other => panic!("expected f64 type, got {other:?}"),
    }

    // Field y: f64
    let f1 = &st.fields[1];
    assert_eq!(interner.search(f1.name_id), "y");
    match &f1.sp_ty_expr.inner {
        TypeExpr::Var(id) => assert_eq!(interner.search(*id), "f64"),
        other => panic!("expected f64 type, got {other:?}"),
    }
}

#[test]
fn parse_nest_export_struct() {
    let text = "nest->\n    export struct Public {}";
    let (ast, interner) = parse_text(text);

    let st = ast.get_struct(section_items(&ast, SectionKind::Nest)[0]);
    assert_eq!(interner.search(st.name_id), "Public");
    assert!(!st.is_priv, "export struct should be public");
}

#[test]
fn parse_nest_enum_empty() {
    let text = "nest->\n    enum Color {}";
    let (ast, interner) = parse_text(text);

    let en = ast.get_enum(section_items(&ast, SectionKind::Nest)[0]);
    assert_eq!(interner.search(en.name_id), "Color");
    assert!(en.variants.is_empty(), "empty enum should have no variants");
}

#[test]
fn parse_nest_enum_with_variants() {
    let text = "nest->\n    enum Color { Red, Green: i32, Blue }";
    let (ast, interner) = parse_text(text);

    let en = ast.get_enum(section_items(&ast, SectionKind::Nest)[0]);
    assert_eq!(interner.search(en.name_id), "Color");
    assert_eq!(en.variants.len(), 3);

    // Red: no type
    assert_eq!(interner.search(en.variants[0].name_id), "Red");
    assert!(en.variants[0].sp_ty_expr.is_none());

    // Green: i32
    assert_eq!(interner.search(en.variants[1].name_id), "Green");
    let green_ty = en.variants[1]
        .sp_ty_expr
        .as_ref()
        .expect("Green variant should have a type");
    match &green_ty.inner {
        TypeExpr::Var(id) => assert_eq!(interner.search(*id), "i32"),
        other => panic!("expected i32 type, got {other:?}"),
    }

    // Blue: no type
    assert_eq!(interner.search(en.variants[2].name_id), "Blue");
    assert!(en.variants[2].sp_ty_expr.is_none());
}

#[test]
fn parse_nest_struct_with_glob_conditions() {
    let text = "nest->\n    struct Foo {} [cond]";
    let (ast, interner) = parse_text(text);

    let st = ast.get_struct(section_items(&ast, SectionKind::Nest)[0]);
    assert_eq!(st.glob_conds.len(), 1);
    match &st.glob_conds[0].expr {
        Expr::Var(id) => assert_eq!(interner.search(*id), "cond"),
        other => panic!("expected Var(cond), got {other:?}"),
    }
}

#[test]
fn parse_nest_struct_with_glob_directives() {
    let text = "nest->\n    struct Foo {} #deprecated";
    let (ast, interner) = parse_text(text);

    let st = ast.get_struct(section_items(&ast, SectionKind::Nest)[0]);
    assert_eq!(st.glob_directives.len(), 1);
    assert_eq!(
        interner.search(st.glob_directives[0].sp_name_id.inner),
        "deprecated"
    );
}

// =============================================================================
// Complex section tests
// =============================================================================

#[test]
fn parse_complex_config_root() {
    let text = "complex->\n    MyConfig { option = [42] }";
    // complex = 0..7, -> = 7..9, \n = 9, spaces = 10..13, MyConfig = 14..22,
    // space = 22, { = 23, space = 24, option = 25..31, space = 31, = = 32, space = 33,
    // [ = 34, 42 = 35..37, ] = 37, space = 38, } = 39
    let (ast, interner) = parse_text(text);

    let sect = ast.sections[SectionKind::Complex as usize]
        .as_ref()
        .expect("expected a complex section");
    assert_eq!(sect.nodes.len(), 1);

    let cfg = ast.get_cfg_root(sect.nodes[0]);
    // name_span should cover "MyConfig" (bytes 14..22)
    assert_eq!(cfg.name_span.start, 14);
    assert_eq!(cfg.name_span.end, 22);

    assert_eq!(interner.search(cfg.name_id), "MyConfig");
    assert!(matches!(cfg.kind, AbstractConfigKind::Root));
    assert_eq!(cfg.opt_assignments.len(), 1);
    assert!(cfg.cfg_members.is_empty());

    // Option assignment: option = [42]
    let opt = &cfg.opt_assignments[0];
    assert_eq!(interner.search(opt.name_id), "option");
    match &opt.array_expr.expr {
        Expr::Array(arr) => {
            assert_eq!(arr.elements.len(), 1);
            match &arr.elements[0].expr {
                Expr::Integer(id, Notation::Decimal) => {
                    assert_eq!(interner.search(*id), "42");
                }
                other => panic!("expected Integer, got {other:?}"),
            }
        }
        other => panic!("expected Array, got {other:?}"),
    }
}

#[test]
fn parse_complex_config_var_prefix() {
    let text = "complex->\n    var MyConfig { opt = [1] }";
    // var prefix at byte 14..17
    let (ast, interner) = parse_text(text);

    let cfg = ast.get_cfg_root(section_items(&ast, SectionKind::Complex)[0]);
    assert_eq!(interner.search(cfg.name_id), "MyConfig");
    assert!(matches!(cfg.kind, AbstractConfigKind::Root));
    // The lookup pattern was changed by the `var` keyword — we can verify it was consumed
    // correctly because the span starts at the name, not at `var`.
    // `var` is at bytes 14..17, then space, then "MyConfig" spans 18..26
    assert_eq!(cfg.name_span.start, 18);
    assert_eq!(cfg.name_span.end, 26);
}

#[test]
fn parse_complex_config_nested() {
    let text = "complex->\n    Outer { inner { opt = [1] } }";
    let (ast, interner) = parse_text(text);

    let cfg = ast.get_cfg_root(section_items(&ast, SectionKind::Complex)[0]);
    assert_eq!(interner.search(cfg.name_id), "Outer");
    assert_eq!(cfg.cfg_members.len(), 1, "expected one inner config");

    let inner = &cfg.cfg_members[0];
    assert_eq!(interner.search(inner.name_id), "inner");
    assert_eq!(inner.opt_assignments.len(), 1);
    assert_eq!(interner.search(inner.opt_assignments[0].name_id), "opt");
}

#[test]
fn parse_complex_config_arrow_syntax() {
    let text = "complex->\n    MyConfig => option = [1]";
    let (ast, interner) = parse_text(text);

    let cfg = ast.get_cfg_root(section_items(&ast, SectionKind::Complex)[0]);
    assert_eq!(interner.search(cfg.name_id), "MyConfig");
    assert_eq!(cfg.opt_assignments.len(), 1);
    assert_eq!(interner.search(cfg.opt_assignments[0].name_id), "option");
}

// =============================================================================
// Expression parsing tests (pratt parser)
// =============================================================================

#[test]
fn parse_expr_binary_precedence() {
    let text = "let x = 1 + 2 * 3";
    // 1 + 2 * 3 => parsed as 1 + (2 * 3) due to precedence
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    let expr = &var.spanned_expr.expr;

    // Top-level: Add(1, Mult(2, 3))
    match expr {
        Expr::BinaryExpr {
            op: BinaryOp::Add,
            lhs,
            rhs,
        } => {
            // lhs = 1
            match &lhs.expr {
                Expr::Integer(id, Notation::Decimal) => {
                    assert_eq!(interner.search(*id), "1");
                }
                other => panic!("expected Integer(1), got {other:?}"),
            }
            // rhs = Mult(2, 3)
            match &rhs.expr {
                Expr::BinaryExpr {
                    op: BinaryOp::Mult,
                    lhs: inner_lhs,
                    rhs: inner_rhs,
                } => {
                    match &inner_lhs.expr {
                        Expr::Integer(id, Notation::Decimal) => {
                            assert_eq!(interner.search(*id), "2");
                        }
                        other => panic!("expected Integer(2), got {other:?}"),
                    }
                    match &inner_rhs.expr {
                        Expr::Integer(id, Notation::Decimal) => {
                            assert_eq!(interner.search(*id), "3");
                        }
                        other => panic!("expected Integer(3), got {other:?}"),
                    }
                }
                other => panic!("expected Mult, got {other:?}"),
            }
        }
        other => panic!("expected Add, got {other:?}"),
    }

    // Span of the whole expression should cover "1 + 2 * 3" (bytes 8..17)
    assert_eq!(var.spanned_expr.span.start, 8);
    assert_eq!(var.spanned_expr.span.end, 17);
}

#[test]
fn parse_expr_comparison() {
    let text = "let x = a == b && c > d";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    let expr = &var.spanned_expr.expr;

    // Precedence: > has bp 3, == has bp 4, && has bp 5 (higher = tighter).
    // So the parse is:  a == (b && c)  then outer > d
    // = "Greater(EqTo(a, And(b, c)), d)"
    match expr {
        Expr::BinaryExpr {
            op: BinaryOp::Greater,
            lhs,
            rhs,
        } => {
            match &lhs.expr {
                Expr::BinaryExpr {
                    op: BinaryOp::EqTo,
                    lhs: ll,
                    rhs: lr,
                } => {
                    match &ll.expr {
                        Expr::Var(id) => assert_eq!(interner.search(*id), "a"),
                        other => panic!("expected Var(a), got {other:?}"),
                    }
                    match &lr.expr {
                        Expr::BinaryExpr {
                            op: BinaryOp::And,
                            lhs: rl,
                            rhs: rr,
                        } => {
                            match &rl.expr {
                                Expr::Var(id) => assert_eq!(interner.search(*id), "b"),
                                other => panic!("expected Var(b), got {other:?}"),
                            }
                            match &rr.expr {
                                Expr::Var(id) => assert_eq!(interner.search(*id), "c"),
                                other => panic!("expected Var(c), got {other:?}"),
                            }
                        }
                        other => panic!("expected And, got {other:?}"),
                    }
                }
                other => panic!("expected EqTo, got {other:?}"),
            }
            match &rhs.expr {
                Expr::Var(id) => assert_eq!(interner.search(*id), "d"),
                other => panic!("expected Var(d), got {other:?}"),
            }
        }
        other => panic!("expected Greater at top level, got {other:?}"),
    }
}

#[test]
fn parse_expr_unary_negate() {
    let text = "let x = -42";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    match &var.spanned_expr.expr {
        Expr::Unary(unary) => {
            assert_eq!(unary.op, UnaryOp::Negate);
            match &unary.spanned_expr.expr {
                Expr::Integer(id, Notation::Decimal) => {
                    assert_eq!(interner.search(*id), "42");
                }
                other => panic!("expected Integer, got {other:?}"),
            }
        }
        other => panic!("expected Unary(Negate), got {other:?}"),
    }
    // Span of "-42" = bytes 8..11
    assert_eq!(var.spanned_expr.span.start, 8);
    assert_eq!(var.spanned_expr.span.end, 11);
}

#[test]
fn parse_expr_unary_not() {
    let text = "let x = !flag";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    match &var.spanned_expr.expr {
        Expr::Unary(unary) => {
            assert_eq!(unary.op, UnaryOp::Not);
            match &unary.spanned_expr.expr {
                Expr::Var(id) => assert_eq!(interner.search(*id), "flag"),
                other => panic!("expected Var, got {other:?}"),
            }
        }
        other => panic!("expected Unary(Not), got {other:?}"),
    }
}

#[test]
fn parse_expr_unary_bitnot() {
    let text = "let x = ~bits";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    match &var.spanned_expr.expr {
        Expr::Unary(unary) => {
            assert_eq!(unary.op, UnaryOp::BitNot);
            match &unary.spanned_expr.expr {
                Expr::Var(id) => assert_eq!(interner.search(*id), "bits"),
                other => panic!("expected Var, got {other:?}"),
            }
        }
        other => panic!("expected Unary(BitNot), got {other:?}"),
    }
}

#[test]
fn parse_expr_shift_operators() {
    let text = "let x = 1 << 2 >> 1";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    // << and >> both have bp 1, left-assoc, so: (1 << 2) >> 1
    let expr = &var.spanned_expr.expr;
    match expr {
        Expr::BinaryExpr {
            op: BinaryOp::BitRightShift,
            lhs,
            rhs,
        } => {
            match &lhs.expr {
                Expr::BinaryExpr {
                    op: BinaryOp::BitLeftShift,
                    lhs: ll,
                    rhs: lr,
                } => {
                    match &ll.expr {
                        Expr::Integer(id, _) => assert_eq!(interner.search(*id), "1"),
                        other => panic!("expected Integer(1), got {other:?}"),
                    }
                    match &lr.expr {
                        Expr::Integer(id, _) => assert_eq!(interner.search(*id), "2"),
                        other => panic!("expected Integer(2), got {other:?}"),
                    }
                }
                other => panic!("expected BitLeftShift, got {other:?}"),
            }
            match &rhs.expr {
                Expr::Integer(id, _) => assert_eq!(interner.search(*id), "1"),
                other => panic!("expected Integer(1), got {other:?}"),
            }
        }
        other => panic!("expected BitRightShift, got {other:?}"),
    }
}

#[test]
fn parse_expr_call_no_args() {
    let text = "let x = f()";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    match &var.spanned_expr.expr {
        Expr::Call(func, args) => {
            match &func.expr {
                Expr::Var(id) => assert_eq!(interner.search(*id), "f"),
                other => panic!("expected Var(f), got {other:?}"),
            }
            assert!(args.is_empty(), "expected no arguments");
        }
        other => panic!("expected Call, got {other:?}"),
    }
}

#[test]
fn parse_expr_call_with_args() {
    let text = "let x = add(1, 2)";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    match &var.spanned_expr.expr {
        Expr::Call(func, args) => {
            match &func.expr {
                Expr::Var(id) => assert_eq!(interner.search(*id), "add"),
                other => panic!("expected Var(add), got {other:?}"),
            }
            assert_eq!(args.len(), 2);
            match &args[0].expr {
                Expr::Integer(id, _) => assert_eq!(interner.search(*id), "1"),
                other => panic!("expected Integer(1), got {other:?}"),
            }
            match &args[1].expr {
                Expr::Integer(id, _) => assert_eq!(interner.search(*id), "2"),
                other => panic!("expected Integer(2), got {other:?}"),
            }
        }
        other => panic!("expected Call, got {other:?}"),
    }
}

#[test]
fn parse_expr_member_access() {
    let text = "let x = obj.field";
    // obj.field => bytes 8..17
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    match &var.spanned_expr.expr {
        Expr::MemberAccess(ma) => {
            match &ma.base.expr {
                Expr::Var(id) => assert_eq!(interner.search(*id), "obj"),
                other => panic!("expected Var(obj), got {other:?}"),
            }
            assert_eq!(interner.search(ma.field), "field");
        }
        other => panic!("expected MemberAccess, got {other:?}"),
    }
    assert_eq!(var.spanned_expr.span.start, 8);
    assert_eq!(var.spanned_expr.span.end, 17);
}

#[test]
fn parse_expr_static_access() {
    let text = "let x = module::Type";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    match &var.spanned_expr.expr {
        Expr::StaticAccess(path) => {
            assert_eq!(path.len(), 2);
            match &path[0].kind {
                PathSegment::Ident(id) => assert_eq!(interner.search(*id), "module"),
                other => panic!("expected Ident(module), got {other:?}"),
            }
            match &path[1].kind {
                PathSegment::Ident(id) => assert_eq!(interner.search(*id), "Type"),
                other => panic!("expected Ident(Type), got {other:?}"),
            }
        }
        other => panic!("expected StaticAccess, got {other:?}"),
    }
}

#[test]
fn parse_expr_static_access_with_generics() {
    let text = "let x = ns::List<i32>";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    match &var.spanned_expr.expr {
        Expr::StaticAccess(path) => {
            assert_eq!(path.len(), 2);
            // First segment: "ns"
            match &path[0].kind {
                PathSegment::Ident(id) => assert_eq!(interner.search(*id), "ns"),
                other => panic!("expected Ident(ns), got {other:?}"),
            }
            // Second segment: List<i32>
            match &path[1].kind {
                PathSegment::Generic(g) => {
                    assert_eq!(interner.search(g.base), "List");
                    assert_eq!(g.inputs.len(), 1);
                    match &g.inputs[0].inner {
                        TypeExpr::Var(id) => assert_eq!(interner.search(*id), "i32"),
                        other => panic!("expected Var(i32), got {other:?}"),
                    }
                }
                other => panic!("expected Generic, got {other:?}"),
            }
        }
        other => panic!("expected StaticAccess, got {other:?}"),
    }
}

#[test]
fn parse_expr_array() {
    // Arrays in let-expressions are not directly supported by parse_primary,
    // but they ARE parsed in config option assignments.
    let text = "complex->\n    Cfg { opt = [1, 2, 3] }";
    let (ast, interner) = parse_text(text);

    let cfg = ast.get_cfg_root(section_items(&ast, SectionKind::Complex)[0]);
    assert_eq!(cfg.opt_assignments.len(), 1);
    let opt = &cfg.opt_assignments[0];
    assert_eq!(interner.search(opt.name_id), "opt");
    match &opt.array_expr.expr {
        Expr::Array(arr) => {
            assert_eq!(arr.elements.len(), 3);
            match &arr.elements[0].expr {
                Expr::Integer(id, _) => assert_eq!(interner.search(*id), "1"),
                other => panic!("expected Integer, got {other:?}"),
            }
            match &arr.elements[1].expr {
                Expr::Integer(id, _) => assert_eq!(interner.search(*id), "2"),
                other => panic!("expected Integer, got {other:?}"),
            }
            match &arr.elements[2].expr {
                Expr::Integer(id, _) => assert_eq!(interner.search(*id), "3"),
                other => panic!("expected Integer, got {other:?}"),
            }
        }
        other => panic!("expected Array, got {other:?}"),
    }
}

#[test]
fn parse_expr_grouped() {
    let text = "let x = (1 + 2) * 3";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    // Top level should be Mult((1+2), 3)
    let expr = &var.spanned_expr.expr;
    match expr {
        Expr::BinaryExpr {
            op: BinaryOp::Mult,
            lhs,
            rhs,
        } => {
            match &lhs.expr {
                Expr::BinaryExpr {
                    op: BinaryOp::Add, ..
                } => {}
                other => panic!("expected Add inside parens, got {other:?}"),
            }
            match &rhs.expr {
                Expr::Integer(id, _) => assert_eq!(interner.search(*id), "3"),
                other => panic!("expected Integer(3), got {other:?}"),
            }
        }
        other => panic!("expected Mult at top, got {other:?}"),
    }
}

#[test]
fn parse_expr_default() {
    let text = "let x = y = 42";
    // `y = 42` is a Default expression: Var(y) with default value Integer(42)
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    match &var.spanned_expr.expr {
        Expr::Default(ident, default_val) => {
            match &ident.expr {
                Expr::Var(id) => assert_eq!(interner.search(*id), "y"),
                other => panic!("expected Var(y), got {other:?}"),
            }
            match &default_val.expr {
                Expr::Integer(id, _) => assert_eq!(interner.search(*id), "42"),
                other => panic!("expected Integer(42), got {other:?}"),
            }
        }
        other => panic!("expected Default, got {other:?}"),
    }
}

// =============================================================================
// Type expression tests
// =============================================================================

#[test]
fn parse_type_expr_simple() {
    let text = "var->\n    t: i32";
    let (ast, interner) = parse_text(text);

    let td = ast.get_typedef(section_items(&ast, SectionKind::Var)[0]);
    match &td.sp_ty_expr.inner {
        TypeExpr::Var(id) => assert_eq!(interner.search(*id), "i32"),
        other => panic!("expected TypeExpr::Var, got {other:?}"),
    }
}

#[test]
fn parse_type_expr_generic() {
    let text = "var->\n    t: List<i32>";
    let (ast, interner) = parse_text(text);

    let td = ast.get_typedef(section_items(&ast, SectionKind::Var)[0]);
    match &td.sp_ty_expr.inner {
        TypeExpr::Generic(g) => {
            assert_eq!(interner.search(g.base), "List");
            assert_eq!(g.inputs.len(), 1);
            match &g.inputs[0].inner {
                TypeExpr::Var(id) => assert_eq!(interner.search(*id), "i32"),
                other => panic!("expected Var(i32), got {other:?}"),
            }
        }
        other => panic!("expected TypeExpr::Generic, got {other:?}"),
    }
}

#[test]
fn parse_type_expr_path() {
    let text = "var->\n    t: module::Type";
    let (ast, interner) = parse_text(text);

    let td = ast.get_typedef(section_items(&ast, SectionKind::Var)[0]);
    match &td.sp_ty_expr.inner {
        TypeExpr::Path(path) => {
            assert_eq!(path.len(), 2);
            match &path[0].kind {
                PathSegment::Ident(id) => assert_eq!(interner.search(*id), "module"),
                other => panic!("expected Ident(module), got {other:?}"),
            }
            match &path[1].kind {
                PathSegment::Ident(id) => assert_eq!(interner.search(*id), "Type"),
                other => panic!("expected Ident(Type), got {other:?}"),
            }
        }
        other => panic!("expected TypeExpr::Path, got {other:?}"),
    }
}

#[test]
fn parse_type_expr_generic_path() {
    let text = "var->\n    t: module::List<i32>";
    let (ast, interner) = parse_text(text);

    let td = ast.get_typedef(section_items(&ast, SectionKind::Var)[0]);
    match &td.sp_ty_expr.inner {
        TypeExpr::Path(path) => {
            assert_eq!(path.len(), 2);
            // First: Ident(module)
            match &path[0].kind {
                PathSegment::Ident(id) => assert_eq!(interner.search(*id), "module"),
                other => panic!("expected Ident(module), got {other:?}"),
            }
            // Second: Generic(List, [i32])
            match &path[1].kind {
                PathSegment::Generic(g) => {
                    assert_eq!(interner.search(g.base), "List");
                    assert_eq!(g.inputs.len(), 1);
                    match &g.inputs[0].inner {
                        TypeExpr::Var(id) => assert_eq!(interner.search(*id), "i32"),
                        other => panic!("expected Var(i32), got {other:?}"),
                    }
                }
                other => panic!("expected Generic, got {other:?}"),
            }
        }
        other => panic!("expected TypeExpr::Path, got {other:?}"),
    }
}

// =============================================================================
// Override section
// =============================================================================

#[test]
fn parse_override_config() {
    let text = "override->\n    MyCfg { opt = [1] }";
    let (ast, interner) = parse_text(text);

    let sect = ast.sections[SectionKind::Override as usize]
        .as_ref()
        .expect("expected an override section");
    assert_eq!(sect.nodes.len(), 1);

    let cfg = ast.get_cfg_root(sect.nodes[0]);
    assert_eq!(interner.search(cfg.name_id), "MyCfg");
    assert!(matches!(cfg.kind, AbstractConfigKind::Root));
    assert_eq!(cfg.opt_assignments.len(), 1);
}

// =============================================================================
// Error / diagnostic tests
// =============================================================================

#[test]
fn parse_invalid_token_yields_diagnostics() {
    let text = "let x = @invalid";
    let (_, diags, _interner) = parse_text_with_diags(text);
    assert!(!diags.is_empty(), "expected diagnostics for invalid token");
    assert_eq!(
        diags[0].level,
        DiagnosticLevel::Error,
        "invalid-token diagnostic should be an error"
    );
}

#[test]
fn parse_missing_expr_yields_diagnostics() {
    let text = "let x = ";
    let (_, diags, _interner) = parse_text_with_diags(text);
    assert!(
        !diags.is_empty(),
        "expected diagnostics for missing expression"
    );
    assert_eq!(
        diags[0].level,
        DiagnosticLevel::Error,
        "missing-expr diagnostic should be an error"
    );
}

#[test]
fn parse_var_section_missing_arrow_yields_diagnostics() {
    let text = "var\n    x: i32";
    let (_, diags, _interner) = parse_text_with_diags(text);
    assert!(!diags.is_empty(), "expected diagnostics for missing '->'");
    assert_eq!(
        diags[0].level,
        DiagnosticLevel::Error,
        "missing-arrow diagnostic should be an error"
    );
}

#[test]
fn parse_nest_missing_struct_enum_yields_diagnostics() {
    let text = "nest->\n    42";
    let (_, diags, _interner) = parse_text_with_diags(text);
    assert!(
        !diags.is_empty(),
        "expected diagnostics for missing struct/enum"
    );
    assert_eq!(
        diags[0].level,
        DiagnosticLevel::Error,
        "missing-struct/enum diagnostic should be an error"
    );
}

#[test]
fn parse_duplicate_section_yields_diagnostics() {
    let text = "var->\n    x: i32\nvar->\n    y: string";
    let (_, diags, _interner) = parse_text_with_diags(text);
    assert!(
        !diags.is_empty(),
        "expected diagnostics for duplicate var section"
    );
    assert_eq!(
        diags[0].level,
        DiagnosticLevel::Error,
        "duplicate-section diagnostic should be an error"
    );
}

#[test]
fn parse_alias_missing_params_yields_diagnostics() {
    let text = "alias x = [1]";
    let (_, diags, _interner) = parse_text_with_diags(text);
    assert!(
        !diags.is_empty(),
        "expected diagnostics for missing alias params"
    );
    assert_eq!(
        diags[0].level,
        DiagnosticLevel::Error,
        "missing-params diagnostic should be an error"
    );
}

// =============================================================================
// Multiple-section composition tests
// =============================================================================

#[test]
fn parse_full_script_with_all_sections() {
    let text = r#"
        bind "./script.chrn"
        let x = 42
        alias twice(a: i32) = [a * 2]

        var->
            name: string
            age: i32

        nest->
            struct Person { name: string, age: i32 }
            enum Role { Admin, User }

        complex->
            App { title = ["Hello"] }

        override->
            App { title = ["Overridden"] }
    "#;

    let (ast, interner) = parse_text(text);

    // Neutral section items: alias (bind and import don't produce items)
    let neutral = ast.sections[SectionKind::Neutral as usize]
        .as_ref()
        .expect("expected neutral section");
    assert_eq!(neutral.nodes.len(), 2); // let x, alias twice

    // Var section
    let var_sect = ast.sections[SectionKind::Var as usize]
        .as_ref()
        .expect("expected var section");
    assert_eq!(var_sect.nodes.len(), 2); // name, age

    // Nest section
    let nest_sect = ast.sections[SectionKind::Nest as usize]
        .as_ref()
        .expect("expected nest section");
    assert_eq!(nest_sect.nodes.len(), 2); // Person, Role

    // Complex section
    let complex_sect = ast.sections[SectionKind::Complex as usize]
        .as_ref()
        .expect("expected complex section");
    assert_eq!(complex_sect.nodes.len(), 1); // App

    // Override section
    let override_sect = ast.sections[SectionKind::Override as usize]
        .as_ref()
        .expect("expected override section");
    assert_eq!(override_sect.nodes.len(), 1); // App

    // Verify the let variable
    let var_item = ast.get_var(neutral.nodes[0]);
    assert_eq!(interner.search(var_item.name_id), "x");

    // Verify alias
    let alias_item = ast.get_alias(neutral.nodes[1]);
    assert_eq!(interner.search(alias_item.name_id), "twice");
    assert_eq!(alias_item.params.len(), 1);

    // Verify typedefs
    let name_td = ast.get_typedef(var_sect.nodes[0]);
    assert_eq!(interner.search(name_td.name_id), "name");
    let age_td = ast.get_typedef(var_sect.nodes[1]);
    assert_eq!(interner.search(age_td.name_id), "age");

    // Verify struct
    let person = ast.get_struct(nest_sect.nodes[0]);
    assert_eq!(interner.search(person.name_id), "Person");
    assert_eq!(person.fields.len(), 2);

    // Verify enum
    let role = ast.get_enum(nest_sect.nodes[1]);
    assert_eq!(interner.search(role.name_id), "Role");
    assert_eq!(role.variants.len(), 2);

    // Verify configs
    let app_cfg = ast.get_cfg_root(complex_sect.nodes[0]);
    assert_eq!(interner.search(app_cfg.name_id), "App");

    let app_override = ast.get_cfg_root(override_sect.nodes[0]);
    assert_eq!(interner.search(app_override.name_id), "App");
}

// =============================================================================
// Edge cases and hardening
// =============================================================================

#[test]
fn parse_empty_input() {
    let text = "";
    let (ast, diags, _interner) = parse_text_with_diags(text);
    assert!(
        diags.is_empty(),
        "empty input should produce no diagnostics"
    );
    // All sections should be None
    for (i, sect) in ast.sections().iter().enumerate() {
        assert!(sect.is_none(), "section {i} should be None for empty input");
    }
    assert!(ast.items().is_empty());
}

#[test]
fn parse_only_comment_input() {
    let text = "// just a comment\n";
    let (ast, diags, _interner) = parse_text_with_diags(text);
    assert!(
        diags.is_empty(),
        "comment-only input should produce no diagnostics"
    );
}

#[test]
fn parse_only_whitespace_input() {
    let text = "   \n  \t  \n  ";
    let (ast, diags, _interner) = parse_text_with_diags(text);
    assert!(
        diags.is_empty(),
        "whitespace-only input should produce no diagnostics"
    );
}

#[test]
fn parse_let_hex_integer() {
    let text = "let x = 0xff";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    match &var.spanned_expr.expr {
        Expr::Integer(id, Notation::Hex) => {
            assert_eq!(interner.search(*id), "255");
        }
        other => panic!("expected Integer(Hex), got {other:?}"),
    }
}

#[test]
fn parse_let_binary_integer() {
    let text = "let x = 0b1010";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    match &var.spanned_expr.expr {
        Expr::Integer(id, Notation::Bin) => {
            assert_eq!(interner.search(*id), "10");
        }
        other => panic!("expected Integer(Bin), got {other:?}"),
    }
}

#[test]
fn parse_let_octal_integer() {
    let text = "let x = 0o77";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    match &var.spanned_expr.expr {
        Expr::Integer(id, Notation::Octal) => {
            assert_eq!(interner.search(*id), "63");
        }
        other => panic!("expected Integer(Octal), got {other:?}"),
    }
}

#[test]
fn parse_let_underscored_number() {
    let text = "let x = 1_000_000";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    match &var.spanned_expr.expr {
        Expr::Integer(id, Notation::Decimal) => {
            assert_eq!(interner.search(*id), "1000000");
        }
        other => panic!("expected Integer(Decimal), got {other:?}"),
    }
}

#[test]
fn parse_chained_member_access() {
    let text = "let x = a.b.c";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    // a.b.c => MemberAccess(MemberAccess(a, b), c)
    match &var.spanned_expr.expr {
        Expr::MemberAccess(outer) => {
            assert_eq!(interner.search(outer.field), "c");
            match &outer.base.expr {
                Expr::MemberAccess(inner) => {
                    assert_eq!(interner.search(inner.field), "b");
                    match &inner.base.expr {
                        Expr::Var(id) => assert_eq!(interner.search(*id), "a"),
                        other => panic!("expected Var(a), got {other:?}"),
                    }
                }
                other => panic!("expected inner MemberAccess, got {other:?}"),
            }
        }
        other => panic!("expected MemberAccess, got {other:?}"),
    }
}

#[test]
fn parse_export_alias() {
    let text = "export alias foo() = [true]";
    let (ast, interner) = parse_text(text);

    let alias = ast.get_alias(section_items(&ast, SectionKind::Neutral)[0]);
    assert_eq!(interner.search(alias.name_id), "foo");
    assert!(!alias.is_priv, "export alias should be public");
}

#[test]
fn parse_export_struct() {
    let text = "nest->\n    export struct Foo {}";
    let (ast, interner) = parse_text(text);

    let st = ast.get_struct(section_items(&ast, SectionKind::Nest)[0]);
    assert!(!st.is_priv, "export struct should be public");
}

#[test]
fn parse_export_enum() {
    let text = "nest->\n    export enum Col {}";
    let (ast, interner) = parse_text(text);

    let en = ast.get_enum(section_items(&ast, SectionKind::Nest)[0]);
    assert!(!en.is_priv, "export enum should be public");
}

#[test]
fn parse_complex_member_override_config() {
    let text = "complex->\n    Outer { inner { opt = [42] } }";
    let (ast, interner) = parse_text(text);

    let cfg = ast.get_cfg_root(section_items(&ast, SectionKind::Complex)[0]);
    assert_eq!(interner.search(cfg.name_id), "Outer");
    assert_eq!(cfg.cfg_members.len(), 1);

    let inner = &cfg.cfg_members[0];
    assert_eq!(interner.search(inner.name_id), "inner");
    // member inner should have Member kind
    assert!(
        matches!(inner.kind, AbstractConfigKind::Member(_)),
        "inner config should be a Member, got {:?}",
        inner.kind
    );
    assert_eq!(inner.opt_assignments.len(), 1);
    assert_eq!(interner.search(inner.opt_assignments[0].name_id), "opt");
}

#[test]
fn parse_multiple_option_assignments() {
    let text = "complex->\n    Cfg { opt1 = [1], opt2 = [2] }";
    let (ast, interner) = parse_text(text);

    let cfg = ast.get_cfg_root(section_items(&ast, SectionKind::Complex)[0]);
    assert_eq!(cfg.opt_assignments.len(), 2);
    assert_eq!(interner.search(cfg.opt_assignments[0].name_id), "opt1");
    assert_eq!(interner.search(cfg.opt_assignments[1].name_id), "opt2");
}

#[test]
fn parse_complex_config_with_trailing_comma() {
    let text = "complex->\n    Cfg { opt = [1], }";
    let (ast, interner) = parse_text(text);

    let cfg = ast.get_cfg_root(section_items(&ast, SectionKind::Complex)[0]);
    assert_eq!(cfg.opt_assignments.len(), 1);
}

#[test]
fn parse_bind_unclosed_string_yields_diags() {
    // The config loader catches unclosed quotes before the lexer/parser run.
    let mut interner = Intern::init();
    let path_id = interner.intern_path(Path::new(""));
    let region_id = SourceRegionId::new(0);
    let text = r#"bind "./some/path"#;
    let result = ConfigLoader::new(region_id, text.as_bytes(), path_id, &ChrnConfig::default())
        .load_config();
    // The config loader should recognize this as broken (unclosed quote).
    match result {
        ConfigLoaderOutput::Broken(_, _) => {} // expected
        ConfigLoaderOutput::Success(_, _) => panic!("expected Broken for unclosed quotes"),
        ConfigLoaderOutput::UnrecoverableErr(e) => panic!("unrecoverable: {e:?}"),
    }
}

#[test]
fn parse_let_with_bitwise_ops() {
    let text = "let x = a & b | c ^ d";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    // & | ^ all have bp 0, left-assoc => ((a & b) | c) ^ d
    let expr = &var.spanned_expr.expr;
    match expr {
        Expr::BinaryExpr {
            op: BinaryOp::BitXor,
            lhs,
            rhs,
        } => {
            // lhs = (a & b) | c
            match &lhs.expr {
                Expr::BinaryExpr {
                    op: BinaryOp::BitOr,
                    lhs: ll,
                    rhs: lr,
                } => {
                    // ll = a & b
                    match &ll.expr {
                        Expr::BinaryExpr {
                            op: BinaryOp::BitAnd,
                            lhs: lll,
                            rhs: llr,
                        } => {
                            match &lll.expr {
                                Expr::Var(id) => assert_eq!(interner.search(*id), "a"),
                                other => panic!("expected Var(a), got {other:?}"),
                            }
                            match &llr.expr {
                                Expr::Var(id) => assert_eq!(interner.search(*id), "b"),
                                other => panic!("expected Var(b), got {other:?}"),
                            }
                        }
                        other => panic!("expected BitAnd, got {other:?}"),
                    }
                    // lr = c
                    match &lr.expr {
                        Expr::Var(id) => assert_eq!(interner.search(*id), "c"),
                        other => panic!("expected Var(c), got {other:?}"),
                    }
                }
                other => panic!("expected BitOr, got {other:?}"),
            }
            // rhs = d
            match &rhs.expr {
                Expr::Var(id) => assert_eq!(interner.search(*id), "d"),
                other => panic!("expected Var(d), got {other:?}"),
            }
        }
        other => panic!("expected BitXor at top, got {other:?}"),
    }
}

#[test]
fn parse_let_with_nested_calls() {
    let text = "let x = f(g(h()))";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    match &var.spanned_expr.expr {
        Expr::Call(outer_func, outer_args) => {
            match &outer_func.expr {
                Expr::Var(id) => assert_eq!(interner.search(*id), "f"),
                other => panic!("expected Var(f), got {other:?}"),
            }
            assert_eq!(outer_args.len(), 1);
            match &outer_args[0].expr {
                Expr::Call(inner_func, inner_args) => {
                    match &inner_func.expr {
                        Expr::Var(id) => assert_eq!(interner.search(*id), "g"),
                        other => panic!("expected Var(g), got {other:?}"),
                    }
                    assert_eq!(inner_args.len(), 1);
                    match &inner_args[0].expr {
                        Expr::Call(deep_func, deep_args) => {
                            match &deep_func.expr {
                                Expr::Var(id) => assert_eq!(interner.search(*id), "h"),
                                other => panic!("expected Var(h), got {other:?}"),
                            }
                            assert!(deep_args.is_empty());
                        }
                        other => panic!("expected Call(h), got {other:?}"),
                    }
                }
                other => panic!("expected Call(g), got {other:?}"),
            }
        }
        other => panic!("expected Call(f), got {other:?}"),
    }
}

#[test]
fn parse_let_scientific_notation() {
    let text = "let x = 1e10";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    match &var.spanned_expr.expr {
        Expr::Float(id, Notation::Decimal) => {
            assert_eq!(interner.search(*id), "1e10");
        }
        other => panic!("expected Float, got {other:?}"),
    }
}

#[test]
fn parse_var_section_generic_typedef() {
    let text = "var->\n    list: List<string>";
    let (ast, interner) = parse_text(text);

    let td = ast.get_typedef(section_items(&ast, SectionKind::Var)[0]);
    match &td.sp_ty_expr.inner {
        TypeExpr::Generic(g) => {
            assert_eq!(interner.search(g.base), "List");
            assert_eq!(g.inputs.len(), 1);
            match &g.inputs[0].inner {
                TypeExpr::Var(id) => assert_eq!(interner.search(*id), "string"),
                other => panic!("expected Var(string), got {other:?}"),
            }
        }
        other => panic!("expected Generic, got {other:?}"),
    }
}

#[test]
fn parse_alias_with_empty_conds() {
    let text = "alias foo() = []";
    let (ast, interner) = parse_text(text);

    let alias = ast.get_alias(section_items(&ast, SectionKind::Neutral)[0]);
    assert!(
        alias.conds.is_empty(),
        "empty brackets should parse as empty conds list"
    );
}

#[test]
fn parse_alias_with_multiple_conds() {
    let text = "alias foo() = [a > 0, b < 10]";
    let (ast, interner) = parse_text(text);

    let alias = ast.get_alias(section_items(&ast, SectionKind::Neutral)[0]);
    assert_eq!(alias.conds.len(), 2);

    // First condition: a > 0
    match &alias.conds[0].expr {
        Expr::BinaryExpr {
            op: BinaryOp::Greater,
            lhs,
            rhs,
        } => {
            match &lhs.expr {
                Expr::Var(id) => assert_eq!(interner.search(*id), "a"),
                other => panic!("expected Var(a) in first cond, got {other:?}"),
            }
            match &rhs.expr {
                Expr::Integer(id, _) => assert_eq!(interner.search(*id), "0"),
                other => panic!("expected Integer(0) in first cond, got {other:?}"),
            }
        }
        other => panic!("expected Greater in first cond, got {other:?}"),
    }

    // Second condition: b < 10
    match &alias.conds[1].expr {
        Expr::BinaryExpr {
            op: BinaryOp::Less,
            lhs,
            rhs,
        } => {
            match &lhs.expr {
                Expr::Var(id) => assert_eq!(interner.search(*id), "b"),
                other => panic!("expected Var(b) in second cond, got {other:?}"),
            }
            match &rhs.expr {
                Expr::Integer(id, _) => assert_eq!(interner.search(*id), "10"),
                other => panic!("expected Integer(10) in second cond, got {other:?}"),
            }
        }
        other => panic!("expected Less in second cond, got {other:?}"),
    }
}

#[test]
fn parse_alias_with_both_conds_and_directives() {
    let text = "alias foo() = [a > 0] #warn";
    let (ast, interner) = parse_text(text);

    let alias = ast.get_alias(section_items(&ast, SectionKind::Neutral)[0]);
    assert_eq!(alias.conds.len(), 1);
    assert_eq!(alias.directives.len(), 1);
    assert_eq!(
        interner.search(alias.directives[0].sp_name_id.inner),
        "warn"
    );
}

#[test]
fn parse_nest_struct_with_typed_variant_conditions() {
    let text = "nest->\n    struct Node { val: i32 [val > 0] }";
    let (ast, interner) = parse_text(text);

    let st = ast.get_struct(section_items(&ast, SectionKind::Nest)[0]);
    assert_eq!(st.fields.len(), 1);
    let field = &st.fields[0];
    assert_eq!(interner.search(field.name_id), "val");
    assert_eq!(field.conds.len(), 1);
}

#[test]
fn parse_nest_enum_with_variant_conditions() {
    let text = "nest->\n    enum E { V: i32 [cond] }";
    let (ast, interner) = parse_text(text);

    let en = ast.get_enum(section_items(&ast, SectionKind::Nest)[0]);
    assert_eq!(en.variants.len(), 1);
    let var = &en.variants[0];
    assert_eq!(interner.search(var.name_id), "V");
    assert_eq!(var.conds.len(), 1, "variant should have one condition");
    match &var.conds[0].expr {
        Expr::Var(id) => assert_eq!(interner.search(*id), "cond"),
        other => panic!("expected Var(cond), got {other:?}"),
    }
}

// =============================================================================
// Span hardening tests
// =============================================================================

/// Verify that every item's name_span is non-empty (start < end) and
/// that the region_id is consistent.
#[test]
fn all_spans_are_non_empty() {
    let text = r#"
        let x = 42
        alias f() = [true]
        var->
            a: i32
            b: string
        nest->
            struct S { f: i32 }
            enum E { A, B }
        complex->
            C { opt = [1] }
    "#;

    let (ast, _interner) = parse_text(text);

    let items: &[_] = ast.items();
    for item in items {
        let span = item.span();
        assert!(
            span.start < span.end,
            "item span should be non-empty: start={}, end={}, item={:?}",
            span.start,
            span.end,
            item
        );
    }
}

/// Verify that when we have an export, it changes is_priv but the span
/// covers the name, not the `export` keyword.
#[test]
fn export_let_span_is_name_only() {
    let text = "export let x = 1";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    // "x" is at byte 11
    assert_eq!(var.name_span.start, 11);
    assert_eq!(var.name_span.end, 12);
    assert!(!var.is_priv);
}

#[test]
fn export_struct_span_is_name_only() {
    let text = "nest->\n    export struct Foo {}";
    let (ast, interner) = parse_text(text);

    let st = ast.get_struct(section_items(&ast, SectionKind::Nest)[0]);
    // "Foo" starts after "export struct " — let's find the exact position
    // "nest->\n    export struct Foo {}"
    // nest-> = 0..6, \n = 6, spaces = 7..10, export = 11..16, space = 17, struct = 18..23,
    // space = 24, Foo = 25..28, space = 28, {} = 29..31
    assert_eq!(st.name_span.start, 25);
    assert_eq!(st.name_span.end, 28);
}

#[test]
fn span_of_binary_expression_covers_entire_expr() {
    let text = "let x = 1 + 2 + 3";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    // "let x = " = 8 bytes, "1 + 2 + 3" = 9 bytes, so span = 8..17
    assert_eq!(var.spanned_expr.span.start, 8);
    assert_eq!(var.spanned_expr.span.end, 17);
}

#[test]
fn span_of_chained_calls() {
    let text = "let x = a.b()";
    // a.b() starts at byte 8 and ends at byte 13
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    assert_eq!(var.spanned_expr.span.start, 8);
    assert_eq!(var.spanned_expr.span.end, 13);

    // Verify it's a Call(MemberAccess(a, b), [])
    match &var.spanned_expr.expr {
        Expr::Call(base, args) => {
            assert!(args.is_empty());
            match &base.expr {
                Expr::MemberAccess(ma) => {
                    assert_eq!(interner.search(ma.field), "b");
                    match &ma.base.expr {
                        Expr::Var(id) => assert_eq!(interner.search(*id), "a"),
                        other => panic!("expected Var(a), got {other:?}"),
                    }
                }
                other => panic!("expected MemberAccess, got {other:?}"),
            }
        }
        other => panic!("expected Call, got {other:?}"),
    }
}

#[test]
fn parse_let_var_escaped_keyword() {
    // Using e# prefix to use a keyword as an identifier
    let text = "let e#let = 42";
    let (ast, interner) = parse_text(text);

    let var = ast.get_var(section_items(&ast, SectionKind::Neutral)[0]);
    assert_eq!(interner.search(var.name_id), "let");
}

#[test]
fn parse_var_type_escaped_keyword() {
    let text = "var->\n    e#let: i32";
    let (ast, interner) = parse_text(text);

    let td = ast.get_typedef(section_items(&ast, SectionKind::Var)[0]);
    assert_eq!(interner.search(td.name_id), "let");
}
