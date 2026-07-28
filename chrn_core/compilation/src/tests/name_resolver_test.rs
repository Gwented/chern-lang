use super::helpers::*;

#[test]
fn nameresolver_duplicate_simple_test() {
    // -- NEUTRAL --
    let wrong = "
            let DUPLICATE = 3
            let DUPLICATE = \"Hi\"
            ";

    let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(wrong);

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

    assert!(
        !diags.diags.is_empty(),
        "Expected errors from NamespaceResolver"
    );

    let correct = "
                let ORIGINAL = 2 + 2
                let NEW = \"Hallo\"
            ";

    let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(correct);

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

    assert!(
        diags.diags.is_empty(),
        "NamespaceResolver should have no errors: {:?}",
        diags
    );

    // -- VAR --
    let wrong = "
            var->
                duplicate: i32
                duplicate: i8
            ";

    // Doing this first since if modules were identified during the parsing stage any
    // syntax error within another module would not be reportable since the parser failed.
    let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(wrong);

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

    assert!(
        !diags.diags.is_empty(),
        "Expected errors from NamespaceResolver"
    );

    let correct = "
            var->
                original: u32
                new: i8
            ";

    let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(correct);

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

    assert!(
        diags.diags.is_empty(),
        "NamespaceResolver should have no errors: {:?}",
        diags
    );

    // -- NEST --

    let wrong = "
            nest->
                struct Duplicate {}
                struct Duplicate {}
            ";

    let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(wrong);

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

    assert!(
        !diags.diags.is_empty(),
        "Expected errors from NamespaceResolver"
    );

    let correct = "
            nest->
                struct Original {}
                struct New {}
            ";

    let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(correct);

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

    assert!(
        diags.diags.is_empty(),
        "NamespaceResolver should have no errors: {:?}",
        diags
    );
    //TEST: -- COMPLEX --

    //TEST: -- OVERRIDE --
}
