use super::helpers::*;

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

    // //TEST: -- COMPLEX --
    // let mut interner = mock_interner(0, 1);
    // let settings = ChrnSettings::default();
    //
    // let text = "
    //     complex->
    //
    //     ";
    //
    // let metadata = ConfigLoader::new(Path::new(""), text.as_bytes(), &settings)
    //     .load_config()
    //     .unwrap();
    //
    // // Doing this first since if modules were identified during the parsing stage any
    // // syntax error within another module would not be reportable since the parser failed.
    //
    // let module = Module::mock(metadata);
    //
    // let mut compiler = ScriptCompiler::init(None, HashMap::default(), vec![module]);
    //
    // let module = &compiler.mods[ModuleId::new(0)];
    //
    // let (toks, _) = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
    //     .tokenize(&mut interner);
    //
    // let ast_info = parser::parse(&settings, &module, &toks, &mut interner).0;
    //
    // // Calls `reporter` internally but the path is fake so this fails
    // let env = ResolverEnv::new(&ast_info, region, mod_id);
    // NamespaceResolver::new(
    //     &settings,
    //     &interner,
    //     &mut compiler,
    // )
    // .resolve(&env)
    // .unwrap();
    //
    // let module = &compiler.mods[ModuleId::new(0)];
    //
    // assert_eq!(module.scope_manager.scopes.len(), 1);
    // assert_eq!(
    //     module.scope_manager.scopes[0].scope_type,
    //     ScopeType::Complex
    // );
    //
    // //TEST: -- OVERRIDE --
    // let mut interner = mock_interner(0, 1);
    // let settings = ChrnSettings::default();
    //
    // let text = "
    //     complex->
    //
    //     ";
    //
    // let metadata = ConfigLoader::new(Path::new(""), text.as_bytes(), &settings)
    //     .load_config()
    //     .unwrap();
    //
    // // Doing this first since if modules were identified during the parsing stage any
    // // syntax error within another module would not be reportable since the parser failed.
    //
    // let module = Module::mock(metadata);
    //
    // let mut compiler = ScriptCompiler::init(None, HashMap::default(), vec![module]);
    //
    // let module = &compiler.mods[ModuleId::new(0)];
    //
    // let (toks, _) = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
    //     .tokenize(&mut interner);
    //
    // let ast_info = parser::parse(&settings, &module, &toks, &mut interner).0;
    //
    // // Calls `reporter` internally but the path is fake so this fails
    // let env = ResolverEnv::new(&ast_info, region, mod_id);
    // NamespaceResolver::new(
    //     &settings,
    //     &interner,
    //     &mut compiler,
    // )
    // .resolve(&env)
    // .unwrap();
    //
    // let module = &compiler.mods[ModuleId::new(0)];
    //
    // assert_eq!(module.scope_manager.scopes.len(), 1);
    // assert_eq!(
    //     module.scope_manager.scopes[0].scope_type,
    //     ScopeType::Override
    // );

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
