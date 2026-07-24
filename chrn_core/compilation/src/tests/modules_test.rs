use crate::config_loader::ConfigLoader;

use super::helpers::*;

#[test]
fn module_simple_test() {
    // -- NEUTRAL --
    let mut interner = mock_interner(2, 2);
    let settings = ChrnConfig::default();

    let main_txt = "
            let CONSTANT = 3
        ";

    let main_region_id = SourceRegionId::new(0);
    let main_meta = ConfigLoader::new(
        main_region_id,
        main_txt.as_bytes(),
        Default::default(),
        &Default::default(),
        &mut interner,
    )
    .load_config()
    .expect_success();

    // Doing this first since if modules were identified during the parsing stage any
    // syntax error within another module would not be reportable since the parser failed.

    let import_path_id = PathId::new(1);
    let import_name_id = InternedId::new(1);
    let kind = ImportKind::Source(import_path_id, Default::default());
    let sub_import = Import::new(import_name_id, ModuleId::new(1), kind, None);

    let main_mod_name_id = InternedId::new(0);
    let main_mod_id = ModuleId::new(0);

    let main_mod = Module::new(
        main_mod_name_id,
        ModuleState::Loaded,
        main_mod_id,
        None,
        vec![sub_import],
        Some(main_region_id),
    );

    let sub_txt = "
            let OTHER_CONSTANT = 5
        ";

    let sub_region_id = SourceRegionId::new(1);
    let sub_meta = ConfigLoader::new(
        sub_region_id,
        sub_txt.as_bytes(),
        import_path_id,
        &settings,
        &mut interner,
    )
    .load_config()
    .expect_success();

    let sub_mod_name_id = InternedId::new(1);
    let sub_mod_id = ModuleId::new(1);

    let sub_mod = Module::new(
        sub_mod_name_id,
        ModuleState::Loaded,
        sub_mod_id,
        Default::default(),
        Default::default(),
        Some(sub_region_id),
    );

    let mut region_arena = Arena::<SourceRegion, SourceRegionId>::new();
    region_arena.push(main_meta);
    region_arena.push(sub_meta);

    let mut compiler = ScriptCompiler::init(None, vec![main_mod, sub_mod].into());

    let mut asts: Vec<Option<AstInfo>> = Vec::new();

    for mod_idx in 0..compiler.mods.len() {
        let module = &compiler.mods[ModuleId::new(mod_idx as u32)];
        let region = match module.region_id {
            Some(id) => &region_arena[id],
            None => {
                asts.push(None);
                continue;
            }
        };

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        asts.push(Some(parser::parse(&settings, region, &toks, &interner).0));
    }

    let reg_envs = build_registration_envs(&compiler, &region_arena, &asts);

    let compilation_syms: Vec<Option<Vec<SymbolId>>> = {
        let mut ns_resolver = NamespaceResolver::new(&settings, &interner, &mut compiler);
        let mut symbols = Vec::new();
        for env in reg_envs.iter() {
            if let Some(env) = env {
                let (s, diags) = ns_resolver.resolve(env);
                assert!(diags.is_empty(), "Namespace resolution failed: {:?}", diags);
                symbols.push(Some(s));
            } else {
                symbols.push(None);
            }
        }
        symbols
    };

    let resolver_envs = build_resolver_envs(&compiler, &region_arena, &asts, &compilation_syms);

    run_member_resolver(&settings, &resolver_envs, &interner, &mut compiler);

    {
        let mut ty_resolver = TypeResolver::new(&settings, &interner, &mut compiler);
        for env in resolver_envs.iter() {
            if let Some(env) = env {
                ty_resolver.resolve(env).unwrap();
            }
        }
    }
}

#[test]
fn module_alias_test() {
    let mut interner = Intern::init();

    let main_txt = "
            var->
                reference: sub_alias::Structure
                other_reference: sub_alias::Enumeration
        ";

    let import = mock_import(
        "sub_module",
        "sub_path",
        ModuleId::new(1),
        Some("sub_alias"),
        &mut interner,
    );

    let (main_mod, main_region) = mock_single_module(
        "main",
        "main_path",
        vec![import],
        0,
        main_txt,
        &mut interner,
    );

    let sub_txt = "
            nest->
                export enum Enumeration {}
                export struct Structure {}
        ";

    let (sub_mod, sub_region) = mock_single_module(
        "sub_module",
        "sub_path",
        Default::default(),
        1,
        sub_txt,
        &mut interner,
    );

    let (arena, _, settings, mut compiler) =
        mock_multiple_module_compiler(vec![(main_mod, main_region), (sub_mod, sub_region)]);

    let mut asts: Vec<Option<AstInfo>> = Vec::new();

    for mod_idx in 0..compiler.mods.len() {
        let module = &compiler.mods[ModuleId::new(mod_idx as u32)];
        let region = match module.region_id {
            Some(region_id) => &arena[region_id],
            None => {
                asts.push(None);
                continue;
            }
        };
        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        asts.push(Some(parser::parse(&settings, region, &toks, &interner).0));
    }

    let reg_envs = build_registration_envs(&compiler, &arena, &asts);

    let compilation_syms: Vec<Option<Vec<SymbolId>>> = {
        let mut ns_resolver = NamespaceResolver::new(&settings, &interner, &mut compiler);
        let mut symbols = Vec::new();
        for env in reg_envs.iter() {
            if let Some(env) = env {
                let (s, diags) = ns_resolver.resolve(env);
                assert!(diags.is_empty(), "Namespace resolution failed: {:?}", diags);
                symbols.push(Some(s));
            } else {
                symbols.push(None);
            }
        }
        symbols
    };

    let resolver_envs = build_resolver_envs(&compiler, &arena, &asts, &compilation_syms);

    run_member_resolver(&settings, &resolver_envs, &interner, &mut compiler);

    {
        let mut ty_resolver = TypeResolver::new(&settings, &interner, &mut compiler);
        for env in resolver_envs.iter() {
            if let Some(env) = env {
                ty_resolver.resolve(env).unwrap();
            }
        }
    }
}

#[test]
fn module_privacy_test() {
    // -- PRIVATE AND FAILING --
    let mut interner = Intern::init();

    let main_txt = "
            var->
                reference: sub_module::Structure
                other_reference: sub_module::Enumeration
        ";

    let import = mock_import(
        "sub_module",
        "sub_path",
        ModuleId::new(1),
        Some("sub_alias"),
        &mut interner,
    );

    let (main_mod, main_region) = mock_single_module(
        "main",
        "main_path",
        vec![import],
        0,
        main_txt,
        &mut interner,
    );

    let sub_txt = "
            nest->
                enum Enumeration {}
                struct Structure {}
        ";

    let (sub_mod, sub_region) = mock_single_module(
        "sub_module",
        "sub_path",
        Default::default(),
        1,
        sub_txt,
        &mut interner,
    );

    let (arena, _, settings, mut compiler) =
        mock_multiple_module_compiler(vec![(main_mod, main_region), (sub_mod, sub_region)]);

    let mut asts: Vec<Option<AstInfo>> = Vec::new();

    for mod_idx in 0..compiler.mods.len() {
        let module = &compiler.mods[ModuleId::new(mod_idx as u32)];
        let region = match module.region_id {
            Some(region_id) => &arena[region_id],
            None => {
                asts.push(None);
                continue;
            }
        };
        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        asts.push(Some(parser::parse(&settings, region, &toks, &interner).0));
    }

    let reg_envs = build_registration_envs(&compiler, &arena, &asts);

    let compilation_syms: Vec<Option<Vec<SymbolId>>> = {
        let mut ns_resolver = NamespaceResolver::new(&settings, &interner, &mut compiler);
        let mut symbols = Vec::new();
        for env in reg_envs.iter() {
            if let Some(env) = env {
                let (s, diags) = ns_resolver.resolve(env);
                assert!(diags.is_empty(), "Namespace resolution failed: {:?}", diags);
                symbols.push(Some(s));
            } else {
                symbols.push(None);
            }
        }
        symbols
    };

    let resolver_envs = build_resolver_envs(&compiler, &arena, &asts, &compilation_syms);

    run_member_resolver(&settings, &resolver_envs, &interner, &mut compiler);

    let mut results = Vec::new();

    {
        let mut ty_resolver = TypeResolver::new(&settings, &interner, &mut compiler);
        for env in resolver_envs.iter() {
            if let Some(env) = env {
                results.push(ty_resolver.resolve(env));
            }
        }
    }

    assert_eq!(results[0].is_err(), true, "Not exported");
    assert_eq!(results[1].is_ok(), true, "Is fine in own context");

    // -- PUBLIC AND SUCCEEDING --
    let mut interner = Intern::init();

    let main_txt = "
            var->
                reference: sub_module::Structure
                other_reference: sub_module::Enumeration
        ";

    let import = mock_import(
        "sub_module",
        "sub_path",
        ModuleId::new(1),
        Some("sub_alias"),
        &mut interner,
    );

    let (main_mod, main_region) = mock_single_module(
        "main",
        "main_path",
        vec![import],
        0,
        main_txt,
        &mut interner,
    );

    let sub_txt = "
            nest->
                export enum Enumeration {}
                export struct Structure {}
        ";

    let (sub_mod, sub_region) = mock_single_module(
        "sub_module",
        "sub_path",
        Default::default(),
        1,
        sub_txt,
        &mut interner,
    );

    let (arena, _, settings, mut compiler) =
        mock_multiple_module_compiler(vec![(main_mod, main_region), (sub_mod, sub_region)]);

    let mut asts: Vec<Option<AstInfo>> = Vec::new();

    for mod_idx in 0..compiler.mods.len() {
        let module = &compiler.mods[ModuleId::new(mod_idx as u32)];
        let region = match module.region_id {
            Some(region_id) => &arena[region_id],
            None => {
                asts.push(None);
                continue;
            }
        };
        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        asts.push(Some(parser::parse(&settings, region, &toks, &interner).0));
    }

    let reg_envs = build_registration_envs(&compiler, &arena, &asts);

    let compilation_syms: Vec<Option<Vec<SymbolId>>> = {
        let mut ns_resolver = NamespaceResolver::new(&settings, &interner, &mut compiler);
        let mut symbols = Vec::new();
        for env in reg_envs.iter() {
            if let Some(env) = env {
                let (s, diags) = ns_resolver.resolve(env);
                assert!(diags.is_empty(), "Namespace resolution failed: {:?}", diags);
                symbols.push(Some(s));
            } else {
                symbols.push(None);
            }
        }
        symbols
    };

    let resolver_envs = build_resolver_envs(&compiler, &arena, &asts, &compilation_syms);

    run_member_resolver(&settings, &resolver_envs, &interner, &mut compiler);

    let mut results = Vec::new();

    {
        let mut ty_resolver = TypeResolver::new(&settings, &interner, &mut compiler);
        for env in resolver_envs.iter() {
            if let Some(env) = env {
                results.push(ty_resolver.resolve(env));
            }
        }
    }

    assert_eq!(results[0].is_ok(), true);
    assert_eq!(results[1].is_ok(), true);
}

#[test]
fn const_dependency_cross_module_resolution_test() {
    let approx_eq = |a: f64, b: f64| (a - b).abs() < 1e-9;

    // 1) Basic: sub defines a literal, main references it in an expression.
    let (compiler, interner) =
        compile_and_resolve_cross_module("let RESULT = sub_module::BASE + 3", "let BASE = 5");
    assert!(matches!(
        value_of(&compiler, &interner, "BASE"),
        Value::I64(5)
    ));
    assert!(matches!(
        value_of(&compiler, &interner, "RESULT"),
        Value::I64(8)
    ));

    // 2) Pending resolution: main references a sub constant before it textually appears in
    //    the sub module source. The resolver should retry pending symbols across modules.
    let (compiler, interner) =
        compile_and_resolve_cross_module("let RESULT = sub_module::BASE * 2", "let BASE = 7");
    assert!(matches!(
        value_of(&compiler, &interner, "BASE"),
        Value::I64(7)
    ));
    assert!(matches!(
        value_of(&compiler, &interner, "RESULT"),
        Value::I64(14)
    ));

    // 3) Diamond: one sub literal feeds two branches in main that are later combined.
    //    LEFT = 3 + 1 = 4, RIGHT = 3 * 2 = 6, TOP = 4 + 6 = 10
    let (compiler, interner) = compile_and_resolve_cross_module(
        "\
            let LEFT = sub_module::BASE + 1\n\
            let RIGHT = sub_module::BASE * 2\n\
            let TOP = LEFT + RIGHT\
        ",
        "let BASE = 3",
    );
    assert!(matches!(
        value_of(&compiler, &interner, "LEFT"),
        Value::I64(4)
    ));
    assert!(matches!(
        value_of(&compiler, &interner, "RIGHT"),
        Value::I64(6)
    ));
    assert!(matches!(
        value_of(&compiler, &interner, "TOP"),
        Value::I64(10)
    ));

    // 4) Chain originating in sub, continuing through main.
    let (compiler, interner) = compile_and_resolve_cross_module(
        "\
            let B = sub_module::A + 3\n\
            let C = B * 2\
        ",
        "let A = 5",
    );
    assert!(matches!(value_of(&compiler, &interner, "B"), Value::I64(8)));
    assert!(matches!(
        value_of(&compiler, &interner, "C"),
        Value::I64(16)
    ));

    // 5) Multiple cross-module references in a single expression.
    let (compiler, interner) = compile_and_resolve_cross_module(
        "let SUM = sub_module::X + sub_module::Y",
        "let X = 10\nlet Y = 20",
    );
    assert!(matches!(
        value_of(&compiler, &interner, "X"),
        Value::I64(10)
    ));
    assert!(matches!(
        value_of(&compiler, &interner, "Y"),
        Value::I64(20)
    ));
    assert!(matches!(
        value_of(&compiler, &interner, "SUM"),
        Value::I64(30)
    ));

    // 6) Floating-point cross-module dependency.
    let (compiler, interner) = compile_and_resolve_cross_module(
        "let AREA = sub_module::PI * sub_module::R * sub_module::R",
        "let PI = 3.14\nlet R = 2.0",
    );
    match value_of(&compiler, &interner, "PI") {
        Value::F64(v) => assert!(approx_eq(v, 3.14), "PI was {}", v),
        other => panic!("Expected F64 for PI, got {:?}", other),
    }
    match value_of(&compiler, &interner, "R") {
        Value::F64(v) => assert!(approx_eq(v, 2.0), "R was {}", v),
        other => panic!("Expected F64 for R, got {:?}", other),
    }
    match value_of(&compiler, &interner, "AREA") {
        Value::F64(v) => assert!(approx_eq(v, 12.56), "AREA was {}", v),
        other => panic!("Expected F64 for AREA, got {:?}", other),
    }

    // 7) Bool derived from cross-module numeric comparison with a local literal.
    let (compiler, interner) =
        compile_and_resolve_cross_module("let IS_BIG = sub_module::VAL > 5", "let VAL = 10");
    assert!(matches!(
        value_of(&compiler, &interner, "VAL"),
        Value::I64(10)
    ));
    assert!(matches!(
        value_of(&compiler, &interner, "IS_BIG"),
        Value::Bool(true)
    ));

    // 8) Unary operator on a cross-module reference.
    let (compiler, interner) =
        compile_and_resolve_cross_module("let NEG = -sub_module::BASE", "let BASE = 5");
    assert!(matches!(
        value_of(&compiler, &interner, "BASE"),
        Value::I64(5)
    ));
    assert!(matches!(
        value_of(&compiler, &interner, "NEG"),
        Value::I64(-5)
    ));
}

#[test]
fn const_dependency_cross_module_circular_test() {
    // 1) Bidirectional circular: main references sub, sub references main.
    //    Main: let X = sub_module::Y + 1
    //    Sub:  let Y = main_module::X * 2
    let mut interner = Intern::init();

    let sub_import = mock_import(
        "sub_module",
        "sub_path",
        ModuleId::new(1),
        None,
        &mut interner,
    );
    let main_import = mock_import(
        "main_module",
        "main_path",
        ModuleId::new(0),
        None,
        &mut interner,
    );

    let (main_mod, main_region) = mock_single_module(
        "main",
        "main_path",
        vec![sub_import],
        0,
        "let X = sub_module::Y + 1",
        &mut interner,
    );

    let (sub_mod, sub_region) = mock_single_module(
        "sub_module",
        "sub_path",
        vec![main_import],
        1,
        "let Y = main_module::X * 2",
        &mut interner,
    );

    let (arena, _, settings, mut compiler) =
        mock_multiple_module_compiler(vec![(main_mod, main_region), (sub_mod, sub_region)]);

    let mut asts: Vec<Option<AstInfo>> = Vec::new();
    for mod_idx in 0..compiler.mods.len() {
        let module = &compiler.mods[ModuleId::new(mod_idx as u32)];
        let region = match module.region_id {
            Some(region_id) => &arena[region_id],
            None => {
                asts.push(None);
                continue;
            }
        };
        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);
        asts.push(Some(parser::parse(&settings, region, &toks, &interner).0));
    }

    let reg_envs = build_registration_envs(&compiler, &arena, &asts);

    let compilation_syms: Vec<Option<Vec<SymbolId>>> = {
        let mut ns_resolver = NamespaceResolver::new(&settings, &interner, &mut compiler);
        let mut symbols = Vec::new();
        for env in reg_envs.iter() {
            if let Some(env) = env {
                let (s, diags) = ns_resolver.resolve(env);
                assert!(diags.is_empty(), "Namespace resolution failed: {:?}", diags);
                symbols.push(Some(s));
            } else {
                symbols.push(None);
            }
        }
        symbols
    };

    let resolver_envs = build_resolver_envs(&compiler, &arena, &asts, &compilation_syms);

    run_member_resolver(&settings, &resolver_envs, &interner, &mut compiler);

    let mut results = Vec::new();
    let mut ty_resolver = TypeResolver::new(&settings, &interner, &mut compiler);
    for env in resolver_envs.iter() {
        if let Some(env) = env {
            results.push(ty_resolver.resolve(env));
        }
    }

    assert!(
        results.iter().any(|r| r.is_err()),
        "Cross-module circular dependency should be rejected: {:?}",
        results
    );

    // 2) Cross-module direct cycle: let A = sub::B, let B = main::A
    let mut interner = Intern::init();

    let sub_import = mock_import(
        "sub_module",
        "sub_path",
        ModuleId::new(1),
        None,
        &mut interner,
    );
    let main_import = mock_import(
        "main_module",
        "main_path",
        ModuleId::new(0),
        None,
        &mut interner,
    );

    let (main_mod, main_region) = mock_single_module(
        "main",
        "main_path",
        vec![sub_import],
        0,
        "let A = sub_module::B",
        &mut interner,
    );

    let (sub_mod, sub_region) = mock_single_module(
        "sub_module",
        "sub_path",
        vec![main_import],
        1,
        "let B = main_module::A",
        &mut interner,
    );

    let (arena, _, settings, mut compiler) =
        mock_multiple_module_compiler(vec![(main_mod, main_region), (sub_mod, sub_region)]);

    let mut asts: Vec<Option<AstInfo>> = Vec::new();
    for mod_idx in 0..compiler.mods.len() {
        let module = &compiler.mods[ModuleId::new(mod_idx as u32)];
        let region = match module.region_id {
            Some(region_id) => &arena[region_id],
            None => {
                asts.push(None);
                continue;
            }
        };
        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);
        asts.push(Some(parser::parse(&settings, region, &toks, &interner).0));
    }

    let reg_envs = build_registration_envs(&compiler, &arena, &asts);

    let compilation_syms: Vec<Option<Vec<SymbolId>>> = {
        let mut ns_resolver = NamespaceResolver::new(&settings, &interner, &mut compiler);
        let mut symbols = Vec::new();
        for env in reg_envs.iter() {
            if let Some(env) = env {
                let (s, diags) = ns_resolver.resolve(env);
                assert!(diags.is_empty(), "Namespace resolution failed: {:?}", diags);
                symbols.push(Some(s));
            } else {
                symbols.push(None);
            }
        }
        symbols
    };

    let resolver_envs = build_resolver_envs(&compiler, &arena, &asts, &compilation_syms);

    run_member_resolver(&settings, &resolver_envs, &interner, &mut compiler);

    let mut results = Vec::new();
    let mut ty_resolver = TypeResolver::new(&settings, &interner, &mut compiler);
    for env in resolver_envs.iter() {
        if let Some(env) = env {
            results.push(ty_resolver.resolve(env));
        }
    }

    assert!(
        results.iter().any(|r| r.is_err()),
        "Cross-module direct cycle should be rejected: {:?}",
        results
    );
}
