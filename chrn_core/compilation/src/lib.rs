pub mod constraints;
pub mod lexer;
pub mod lookup;
pub mod modules;
pub mod parser;
pub mod resolvers;
pub mod script_compiler;
pub mod semantic;
pub mod token;

#[cfg(test)]
mod tests {
    use crate::{
        parser::ast::ast_concepts::AstInfo, resolvers::resolver_env::ResolverEnv,
        script_compiler::ScriptCompiler,
    };
    // -- Helpers --
    /// Creates fake strings for the amounts given
    fn mock_interner(str_amt: usize, path_amt: usize) -> Intern {
        let mut interner = Intern::init();

        for idx in 0..str_amt {
            let s = format!("dummyname{idx}");
            interner.intern(&s);
        }

        for idx in 0..path_amt {
            let p = format!("dummyimport{idx}");
            let p = Path::new(&p);
            interner.intern_path(&p);
        }

        interner
    }

    fn get_module_region<'a>(arena: &'a SourceRegionArena, module: &Module) -> &'a SourceRegion {
        let region_id = module
            .region_id
            .expect("Module should have a source region");
        arena.extract_region(region_id)
    }

    fn mock_single_module_compiler(
        text: &str,
    ) -> (SourceRegionArena, Intern, ChrnSettings, ScriptCompiler) {
        let interner = mock_interner(0, 1);
        let settings = ChrnSettings::default();
        let path_id = PathId::new(0);
        let region_id = SourceRegionId::new(0);

        let source_region =
            ChrnConfigLoader::new(region_id, text.as_bytes(), path_id, &settings, &interner)
                .load_config()
                .unwrap();

        let module = Module::new(
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Some(region_id),
        );

        // Should use compiler store now
        let arena = SourceRegionArena::new(vec![source_region]);
        let compiler = ScriptCompiler::init(None, vec![module]);

        (arena, interner, settings, compiler)
    }

    fn mock_import(
        name: &str,
        path_name: &str,
        mod_id: ModuleId,
        alias_id: Option<&str>,
        interner: &mut Intern,
    ) -> Import {
        let kind = ImportKind::Source(
            interner.intern_path(&Path::new(path_name)),
            Default::default(),
        );
        Import::new(
            interner.intern(name),
            mod_id,
            kind,
            alias_id.map(|a| interner.intern(&a)),
        )
    }

    fn mock_single_module(
        name: &str,
        path_name: &str,
        imports: Vec<Import>,
        mod_id: usize,
        text: &str,
        interner: &mut Intern,
    ) -> (Module, SourceRegion) {
        let settings = ChrnSettings::default();
        let path_id = interner.intern_path(Path::new(path_name));
        let region_id = SourceRegionId::new(mod_id as u32);

        let source_region =
            ChrnConfigLoader::new(region_id, text.as_bytes(), path_id, &settings, interner)
                .load_config()
                .unwrap();

        let module = Module::new(
            interner.intern(name),
            ModuleState::Loaded,
            ModuleId::new(mod_id),
            imports,
            Some(region_id),
        );

        (module, source_region)
    }

    fn mock_multiple_module_compiler(
        modules_with_regions: Vec<(Module, SourceRegion)>,
    ) -> (SourceRegionArena, Intern, ChrnSettings, ScriptCompiler) {
        let interner = mock_interner(0, modules_with_regions.len());
        let settings = ChrnSettings::default();

        let (modules, regions): (Vec<Module>, Vec<SourceRegion>) =
            modules_with_regions.into_iter().unzip();

        let arena = SourceRegionArena::new(regions);
        let compiler = ScriptCompiler::init(None, modules);

        (arena, interner, settings, compiler)
    }
    // Builder?
    //fn setup_multiple_modules(text: &str, ) -> (Intern, ChrnSettings, ScriptCompiler) {}

    use std::path::Path;

    use chrn_utils::{
        chrn_settings::ChrnSettings,
        id_types::{InternedId, ModuleId, PathId, SourceRegionId},
        intern::Intern,
        source_map::source_region::{SourceRegion, SourceRegionArena},
    };
    use lang::config_loader::ChrnConfigLoader;

    use crate::{
        lexer::Lexer,
        lookup::scopes::ScopeType,
        modules::{Import, ImportKind, Module, ModuleState},
        parser::{self},
        resolvers::{
            name_resolver::NamespaceResolver,
            type_resolver::{TypeResolver, type_context::TypeContext},
        },
        token::{Notation, Token},
    };

    #[test]
    fn lex_tok_test() {
        let text = r#"bind "./some/path""#;

        let mut interner = Intern::init();
        let path_id = interner.intern_path(Path::new(""));
        let region_id = SourceRegionId::new(0);
        let metadata = ChrnConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .unwrap();

        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(
            None, metadata.serial_start,
            "start_offset without `@def` failed"
        );
        assert_eq!(3, toks.len(), "Token length exceeded 3 in lex_tok_test");
    }

    #[test]
    fn lex_tok_test_rev() {
        // Properly closed @def and @end
        let correct = r#"@defbind "./some/path"@end"#;

        let mut interner = mock_interner(1, 1);
        let path_id = interner.intern_path(Path::new(""));
        let region_id = SourceRegionId::new(0);
        let opt = ChrnConfigLoader::new(
            region_id,
            correct.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config();

        assert_eq!(true, opt.is_ok());

        // Improper @def without an @end
        // This type of error is more likely to break the diagnostic reporting but is fixed for
        // now.
        let wrong = r#"@defbind "./some/path""#;

        let opt = ChrnConfigLoader::new(
            region_id,
            wrong.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config();

        assert_eq!(true, opt.is_err());
    }

    #[test]
    fn char_literal_test() {
        // Valid single character
        let text = "'a'";
        let mut interner = Intern::init();

        let path_id = interner.intern_path(Path::new(""));
        let region_id = SourceRegionId::new(0);

        let metadata = ChrnConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .unwrap();
        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        assert!(
            matches!(toks[0].tok, Token::Char(_),),
            "Expected char token, got {:?}",
            toks[0].tok
        );

        // Valid escaped character
        let text = "'\\n'";
        let mut interner = Intern::init();

        let path_id = interner.intern_path(Path::new(""));
        let region_id = SourceRegionId::new(0);

        let metadata = ChrnConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .unwrap();

        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        assert!(
            matches!(toks[0].tok, Token::Char(_),),
            "Expected char token, got {:?}",
            toks[0].tok
        );

        // Valid hex escape
        let text = "'\\x2F'";

        let mut interner = Intern::init();
        let path_id = interner.intern_path(Path::new(""));
        let region_id = SourceRegionId::new(0);

        let metadata = ChrnConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .unwrap();

        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        assert!(
            matches!(toks[0].tok, Token::Char(_),),
            "Expected char token, got {:?}",
            toks[0].tok
        );

        // Invalid character
        let text = "'aa'";
        let mut interner = Intern::init();
        let path_id = interner.intern_path(Path::new(""));
        let region_id = SourceRegionId::new(0);
        let metadata = ChrnConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .unwrap();
        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        assert!(
            matches!(toks[0].tok, Token::Illegal(_),),
            "Expected Illegal token, got {:?}",
            toks[0].tok
        );

        // Invalid hex escape
        let text = "'\\x2'";
        let mut interner = Intern::init();
        let path_id = interner.intern_path(Path::new(""));
        let region_id = SourceRegionId::new(0);
        let metadata = ChrnConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .unwrap();
        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        assert!(
            matches!(toks[0].tok, Token::Illegal(_),),
            "Expected Illegal token, got {:?}",
            toks[0].tok
        );

        // I can't actually read hex
        // Invalid hex digits
        let text = "'\\x255'";
        let mut interner = Intern::init();
        let path_id = interner.intern_path(Path::new(""));
        let region_id = SourceRegionId::new(0);
        let metadata = ChrnConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::new(),
            &interner,
        )
        .load_config()
        .unwrap();
        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        assert!(
            matches!(toks[0].tok, Token::Illegal(_),),
            "Expected Illegal token, got {:?}",
            toks[0].tok
        );

        // Unknown escape
        let text = "'\\q'";
        let mut interner = Intern::init();
        let path_id = interner.intern_path(Path::new(""));
        let region_id = SourceRegionId::new(0);
        let metadata = ChrnConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::new(),
            &interner,
        )
        .load_config()
        .unwrap();
        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        assert!(
            matches!(toks[0].tok, Token::Illegal(_),),
            "Expected Illegal token, got {:?}",
            toks[0].tok
        );

        // Out of range escape
        let text = "'\\x1Y'";
        let mut interner = Intern::init();
        let path_id = interner.intern_path(Path::new(""));
        let region_id = SourceRegionId::new(0);
        let metadata = ChrnConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::new(),
            &interner,
        )
        .load_config()
        .unwrap();
        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        assert!(
            matches!(toks[0].tok, Token::Illegal(_),),
            "Expected Illegal token, got {:?}",
            toks[0].tok
        );
    }

    #[test]
    fn multi_line_comment_test() {
        // Properly closed multi-line comment
        let correct = "
                /* /* */ */
            "
        .as_bytes();

        // Unclosed multi-line comment
        let wrong = "
                /* /* */
            "
        .as_bytes();

        let mut interner = mock_interner(0, 2);
        let region_id = SourceRegionId::new(0);

        let correct = ChrnConfigLoader::new(
            region_id,
            correct,
            PathId::default(),
            &ChrnSettings::default(),
            &interner,
        )
        .load_config();

        let wrong = ChrnConfigLoader::new(
            region_id,
            wrong,
            PathId::default(),
            &ChrnSettings::default(),
            &interner,
        )
        .load_config();

        assert_eq!(true, correct.is_ok());
        assert_eq!(true, wrong.is_err());
    }

    #[test]
    fn start_and_serial_offset_test() {
        let text = format!("adwh@def var-> int: i32 @endhi");
        let interner = mock_interner(0, 1);
        let region_id = SourceRegionId::new(0);

        let metadata = ChrnConfigLoader::new(
            region_id,
            text.as_bytes(),
            PathId::default(),
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .unwrap();

        assert_eq!(&text[4..], &text[metadata.script_start..]);
        assert_eq!("hi", &text[metadata.serial_start.unwrap()..]);
        assert_eq!(28, metadata.serial_start.unwrap());
    }

    #[test]
    fn lex_notation_test() {
        // Hex Test (Hex Text (Hex Test))
        let text = "0xff";
        let mut interner = mock_interner(1, 1);

        let path_id = PathId::new(0);
        let region_id = SourceRegionId::new(0);

        let metadata = ChrnConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .unwrap();

        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        match toks[0].tok {
            Token::Integer(id, Notation::Hex) => {
                assert_eq!("255", interner.search(id));
            }
            _ => panic!("Expected Integer with Hex, found {:?}", toks[0].tok),
        }

        // Binary
        let text = "0b1010";
        let metadata = ChrnConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .unwrap();
        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        match toks[0].tok {
            Token::Integer(id, Notation::Bin) => {
                assert_eq!("10", interner.search(id));
            }
            _ => panic!("Expected Integer with Binary, found {:?}", toks[0].tok),
        }

        // Octal
        let text = "0o77";
        let metadata = ChrnConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .unwrap();
        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        match toks[0].tok {
            Token::Integer(id, Notation::Octal) => {
                assert_eq!("63", interner.search(id));
            }
            _ => panic!("Expected Integer with Octal, found {:?}", toks[0].tok),
        }

        // Decimal
        let text = "42";
        let metadata = ChrnConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .unwrap();
        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        match toks[0].tok {
            Token::Integer(id, Notation::Decimal) => {
                assert_eq!("42", interner.search(id));
            }
            _ => panic!("Expected Integer of Decimal, found {:?}", toks[0].tok),
        }

        // Float with decimal
        let text = "3.14";
        let metadata = ChrnConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .unwrap();
        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        match toks[0].tok {
            Token::Float(id, Notation::Decimal) => {
                assert_eq!("3.14", interner.search(id));
            }
            _ => panic!("Expected Float with Decimal, found {:?}", toks[0].tok),
        }

        // Positive Scientific Notation
        let text = "1e+23";
        let metadata = ChrnConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .unwrap();
        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        match toks[0].tok {
            Token::Float(id, Notation::Decimal) => {
                assert_eq!("1e+23", interner.search(id));
            }
            _ => panic!("Expected Float with Decimal, found {:?}", toks[0].tok),
        }

        // Negative Scientific Notation
        let text = "1e-23";
        let metadata = ChrnConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .unwrap();
        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        match toks[0].tok {
            Token::Float(id, Notation::Decimal) => {
                assert_eq!("1e-23", interner.search(id));
            }
            _ => panic!("Expected Float with Decimal, found {:?}", toks[0].tok),
        }

        // Underscored Numbers
        let text = "1_000_000";
        let metadata = ChrnConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .unwrap();
        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        match toks[0].tok {
            Token::Integer(id, Notation::Decimal) => {
                assert_eq!("1000000", interner.search(id));
            }
            _ => panic!("Expected Integer with Decimal, found {:?}", toks[0].tok),
        }

        // Underscored Hex
        let text = "0x_ff_ff";
        let metadata = ChrnConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .unwrap();
        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        match toks[0].tok {
            Token::Integer(id, Notation::Hex) => {
                assert_eq!("65535", interner.search(id));
            }
            _ => panic!("Expected Integer with Hex, found {:?}", toks[0].tok),
        }
    }

    #[test]
    fn nameresolver_duplicate_simple_test() {
        // -- NEUTRAL --
        let wrong = "
                let DUPLICATE = 3
                let DUPLICATE = \"Hi\"
                ";

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(wrong);

        let module = &compiler.mods[0];

        let region = get_module_region(&arena, module);
        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let res = NamespaceResolver::new(
            &settings,
            &ast_info,
            region,
            &interner,
            module.mod_id,
            &mut compiler,
        )
        .resolve();

        assert_eq!(res.is_err(), true);

        let correct = "
                    let ORIGINAL = 2 + 2
                    let NEW = \"Hallo\"
                ";

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(correct);

        let module = &compiler.mods[0];

        let region = get_module_region(&arena, module);
        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let res = NamespaceResolver::new(
            &settings,
            &ast_info,
            region,
            &interner,
            module.mod_id,
            &mut compiler,
        )
        .resolve();

        assert_eq!(res.is_ok(), true);

        // -- VAR --
        let wrong = "
                var->
                    duplicate: i32
                    duplicate: i8
                ";

        // Doing this first since if modules were identified during the parsing stage any
        // syntax error within another module would not be reportable since the parser failed.
        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(wrong);

        let module = &compiler.mods[0];

        let region = get_module_region(&arena, module);
        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let res = NamespaceResolver::new(
            &settings,
            &ast_info,
            region,
            &interner,
            module.mod_id,
            &mut compiler,
        )
        .resolve();

        assert_eq!(res.is_err(), true);

        let correct = "
                var->
                    original: u32
                    new: i8
                ";

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(correct);

        let module = &compiler.mods[0];

        let region = get_module_region(&arena, module);
        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let res = NamespaceResolver::new(
            &settings,
            &ast_info,
            region,
            &interner,
            module.mod_id,
            &mut compiler,
        )
        .resolve();

        assert_eq!(res.is_ok(), true);

        // -- NEST --

        let wrong = "
                nest->
                    struct Duplicate {}
                    struct Duplicate {}
                ";

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(wrong);

        let module = &compiler.mods[0];

        let region = get_module_region(&arena, module);
        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let res = NamespaceResolver::new(
            &settings,
            &ast_info,
            region,
            &interner,
            module.mod_id,
            &mut compiler,
        )
        .resolve();

        assert_eq!(res.is_err(), true);

        let correct = "
                nest->
                    struct Original {}
                    struct New {}
                ";

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(correct);

        let module = &compiler.mods[0];

        let region = get_module_region(&arena, module);
        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let res = NamespaceResolver::new(
            &settings,
            &ast_info,
            region,
            &interner,
            module.mod_id,
            &mut compiler,
        )
        .resolve();

        assert_eq!(res.is_ok(), true);
        //TEST: -- COMPLEX --

        //TEST: -- OVERRIDE --
    }

    #[test]
    fn module_simple_test() {
        // -- NEUTRAL --
        let mut interner = mock_interner(2, 2);
        let settings = ChrnSettings::default();

        let main_txt = "
                let CONSTANT = 3
            ";

        let main_region_id = SourceRegionId::new(0);
        let main_meta = ChrnConfigLoader::new(
            main_region_id,
            main_txt.as_bytes(),
            Default::default(),
            &Default::default(),
            &mut interner,
        )
        .load_config()
        .unwrap();

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
            vec![sub_import],
            Some(main_region_id),
        );

        let sub_txt = "
                let OTHER_CONSTANT = 5
            ";

        let sub_region_id = SourceRegionId::new(1);
        let sub_meta = ChrnConfigLoader::new(
            sub_region_id,
            sub_txt.as_bytes(),
            import_path_id,
            &settings,
            &mut interner,
        )
        .load_config()
        .unwrap();

        let sub_mod_name_id = InternedId::new(1);
        let sub_mod_id = ModuleId::new(1);

        let sub_mod = Module::new(
            sub_mod_name_id,
            ModuleState::Loaded,
            sub_mod_id,
            Default::default(),
            Some(sub_region_id),
        );

        let region_arena = SourceRegionArena::new(vec![main_meta, sub_meta]);

        let mut compiler = ScriptCompiler::init(None, vec![main_mod, sub_mod]);

        let mut asts: Vec<AstInfo> = Vec::new();

        for mod_idx in 0..compiler.mods.len() {
            let module = &compiler.mods[mod_idx];
            let region = match module.region_id {
                Some(id) => region_arena.extract_region(id),
                None => continue,
            };

            let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
                .tokenize(&mut interner);

            let ast_info = parser::parse(&settings, region, &toks, &interner).0;

            NamespaceResolver::new(
                &settings,
                &ast_info,
                region,
                &interner,
                module.mod_id,
                &mut compiler,
            )
            .resolve()
            .unwrap();

            asts.push(ast_info);
        }

        for i in 0..compiler.mods.len() {
            let mod_id = ModuleId::new(i);
            let region_id = compiler.mods[mod_id.id].region_id;

            let region = match region_id {
                Some(id) => region_arena.extract_region(id),
                None => continue,
            };

            let env = ResolverEnv::new(&asts[i], region, mod_id);
            TypeResolver::new(&settings, &interner, &mut compiler)
                .resolve(&env)
                .unwrap();
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
            let module = &compiler.mods[mod_idx];
            let region = match module.region_id {
                Some(region_id) => arena.extract_region(region_id),
                None => {
                    asts.push(None);
                    continue;
                }
            };
            let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
                .tokenize(&mut interner);

            let ast_info = parser::parse(&settings, region, &toks, &interner).0;

            NamespaceResolver::new(
                &settings,
                &ast_info,
                region,
                &interner,
                module.mod_id,
                &mut compiler,
            )
            .resolve()
            .unwrap();

            asts.push(Some(ast_info));
        }

        for i in 0..compiler.mods.len() {
            let module = &compiler.mods[i];
            let region = match module.region_id {
                Some(region_id) => arena.extract_region(region_id),
                None => continue,
            };
            let env = ResolverEnv::new(
                asts[i].as_ref().expect("Has metadata already"),
                region,
                module.mod_id,
            );
            TypeResolver::new(&settings, &interner, &mut compiler)
                .resolve(&env)
                .unwrap();
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
            let module = &compiler.mods[mod_idx];
            let region = match module.region_id {
                Some(region_id) => arena.extract_region(region_id),
                None => {
                    asts.push(None);
                    continue;
                }
            };
            let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
                .tokenize(&mut interner);

            let ast_info = parser::parse(&settings, region, &toks, &interner).0;

            NamespaceResolver::new(
                &settings,
                &ast_info,
                region,
                &interner,
                module.mod_id,
                &mut compiler,
            )
            .resolve()
            .unwrap();

            asts.push(Some(ast_info));
        }

        let mut results = Vec::new();

        for i in 0..compiler.mods.len() {
            let module = &compiler.mods[i];
            let region = match module.region_id {
                Some(region_id) => arena.extract_region(region_id),
                None => continue,
            };
            let env = ResolverEnv::new(
                asts[i].as_ref().expect("Has metadata already"),
                region,
                module.mod_id,
            );
            results.push(TypeResolver::new(&settings, &interner, &mut compiler).resolve(&env));
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
            let module = &compiler.mods[mod_idx];
            let region = match module.region_id {
                Some(region_id) => arena.extract_region(region_id),
                None => {
                    asts.push(None);
                    continue;
                }
            };
            let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
                .tokenize(&mut interner);

            let ast_info = parser::parse(&settings, region, &toks, &interner).0;

            NamespaceResolver::new(
                &settings,
                &ast_info,
                region,
                &interner,
                module.mod_id,
                &mut compiler,
            )
            .resolve()
            .unwrap();

            asts.push(Some(ast_info));
        }

        let mut results = Vec::new();

        for i in 0..compiler.mods.len() {
            let module = &compiler.mods[i];
            let region = match module.region_id {
                Some(region_id) => arena.extract_region(region_id),
                None => continue,
            };
            let env = ResolverEnv::new(
                asts[i].as_ref().expect("Has metadata already"),
                region,
                module.mod_id,
            );
            results.push(TypeResolver::new(&settings, &interner, &mut compiler).resolve(&env));
        }

        assert_eq!(results[0].is_ok(), true);
        assert_eq!(results[1].is_ok(), true);
    }

    #[test]
    fn scope_simple_test() {
        // -- NEUTRAL --
        let text = "
                let CONSTANT = 3
                ";

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);

        let module = &compiler.mods[0];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        NamespaceResolver::new(
            &settings,
            &ast_info,
            region,
            &interner,
            module.mod_id,
            &mut compiler,
        )
        .resolve()
        .unwrap();

        let module = &compiler.mods[0];

        assert_eq!(module.scopes.len(), 2);
        assert_eq!(
            compiler.get_scope(module.scopes[1]).scope.scope_type,
            ScopeType::Neutral
        );

        // -- VAR --
        let text = "
                var->
                    variable: i32
                ";

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);

        let module = &compiler.mods[0];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        NamespaceResolver::new(
            &settings,
            &ast_info,
            region,
            &interner,
            module.mod_id,
            &mut compiler,
        )
        .resolve()
        .unwrap();

        let module = &compiler.mods[0];

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

        let module = &compiler.mods[0];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        NamespaceResolver::new(
            &settings,
            &ast_info,
            region,
            &interner,
            module.mod_id,
            &mut compiler,
        )
        .resolve()
        .unwrap();

        let module = &compiler.mods[0];

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
        // let metadata = ChrnConfigLoader::new(Path::new(""), text.as_bytes(), &settings)
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
        // let module = &compiler.mods[0];
        //
        // let (toks, _) = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
        //     .tokenize(&mut interner);
        //
        // let ast_info = parser::parse(&settings, &module, &toks, &mut interner).0;
        //
        // // Calls `reporter` internally but the path is fake so this fails
        // NamespaceResolver::new(
        //     &settings,
        //     &ast_info,
        //     &interner,
        //     module.mod_id,
        //     &mut compiler,
        // )
        // .resolve()
        // .unwrap();
        //
        // let module = &compiler.mods[0];
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
        // let metadata = ChrnConfigLoader::new(Path::new(""), text.as_bytes(), &settings)
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
        // let module = &compiler.mods[0];
        //
        // let (toks, _) = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
        //     .tokenize(&mut interner);
        //
        // let ast_info = parser::parse(&settings, &module, &toks, &mut interner).0;
        //
        // // Calls `reporter` internally but the path is fake so this fails
        // NamespaceResolver::new(
        //     &settings,
        //     &ast_info,
        //     &interner,
        //     module.mod_id,
        //     &mut compiler,
        // )
        // .resolve()
        // .unwrap();
        //
        // let module = &compiler.mods[0];
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

        let module = &compiler.mods[0];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        NamespaceResolver::new(
            &settings,
            &ast_info,
            region,
            &interner,
            module.mod_id,
            &mut compiler,
        )
        .resolve()
        .unwrap();

        //TODO: Override and Complex
        let module = &compiler.mods[0];
        assert_eq!(module.scopes.len(), 4);
        assert_eq!(
            compiler.get_scope(module.scopes[1]).scope.scope_type,
            ScopeType::Neutral
        );
        assert_eq!(
            compiler.get_scope(module.scopes[2]).scope.scope_type,
            ScopeType::Var
        );
        assert_eq!(
            compiler.get_scope(module.scopes[3]).scope.scope_type,
            ScopeType::Nest
        );
    }

    #[test]
    fn type_resolver_simple_test() {
        let wrong = "
                var->
                    primitive: i32
                    undeclared_type: Thing
                ";

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(wrong);

        let module = &compiler.mods[0];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        NamespaceResolver::new(
            &settings,
            &ast_info,
            region,
            &interner,
            module.mod_id,
            &mut compiler,
        )
        .resolve()
        .unwrap();

        let mod_id = compiler.mods[0].mod_id;
        let env = ResolverEnv::new(&ast_info, region, mod_id);
        let res = TypeResolver::new(&settings, &interner, &mut compiler).resolve(&env);

        assert_eq!(res.is_err(), true);

        let correct = "
                var->
                    primitive: i32
                    declared_type: Thing
                nest->
                    struct Thing {}
                ";

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(correct);

        let module = &compiler.mods[0];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        NamespaceResolver::new(
            &settings,
            &ast_info,
            region,
            &interner,
            module.mod_id,
            &mut compiler,
        )
        .resolve()
        .unwrap();

        let mod_id = compiler.mods[0].mod_id;
        let env = ResolverEnv::new(&ast_info, region, mod_id);
        let res = TypeResolver::new(&settings, &interner, &mut compiler).resolve(&env);

        assert_eq!(res.is_ok(), true);
    }

    #[test]
    fn type_resolver_complex_test() {
        let text = "
                let CONSTANT = 4
                ";

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);

        let module = &compiler.mods[0];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        NamespaceResolver::new(
            &settings,
            &ast_info,
            region,
            &interner,
            module.mod_id,
            &mut compiler,
        )
        .resolve()
        .unwrap();

        let env = ResolverEnv::new(&ast_info, region, Default::default());
        TypeResolver::new(&settings, &interner, &mut compiler)
            .resolve(&env)
            .unwrap();

        assert!(compiler.symbols.len() > 0);
    }
    //
    // #[test]
    // fn constraint_resolver_let_test() {
    //     let text = "
    //             let CONSTANT = 4
    //             ";
    //
    //     let (mut interner, settings, mut compiler) = mock_single_module_compiler(text);
    //
    //     let pre_loaded_values = compiler.values.len();
    //
    //     let module = &compiler.mods[0];
    //
    //     let (toks, _) = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
    //         .tokenize(&mut interner);
    //
    //     let ast_info = parser::parse(&settings, &module, &toks, &mut interner).0;
    //
    //     NamespaceResolver::new(
    //         &settings,
    //         &ast_info,
    //         &interner,
    //         module.mod_id,
    //         &mut compiler,
    //     )
    //     .resolve()
    //     .unwrap();
    //
    //     TypeResolver::new(
    //         &settings,
    //         &ast_info,
    //         Default::default(),
    //         &interner,
    //         &mut compiler,
    //     )
    //     .resolve()
    //     .unwrap();
    //
    //     let mut val_ctx = ValueContext::new();
    //
    //     ConstraintResolver::new(
    //         &settings,
    //         &[ast_info],
    //         &interner,
    //         Default::default(),
    //         &mut val_ctx,
    //         &mut compiler,
    //     )
    //     .resolve()
    //     .unwrap();
    //
    //     assert_eq!(compiler.symbols.len(), 1);
    //     assert_eq!(compiler.values.len() - pre_loaded_values, 1);
    //     match &compiler.values[compiler.values.len() - 1]
    //         .const_val
    //         .as_ref()
    //         .unwrap()
    //     {
    //         Value::I128(_) => (),
    //         _ => panic!("Value mistmatch"),
    //     };
    //
    //     let text = "
    //             let CONSTANT = \"Hallo\"
    //         ";
    //
    //     let (mut interner, settings, mut compiler) = mock_single_module_compiler(text);
    //
    //     let pre_loaded_values = compiler.values.len();
    //
    //     let module = &compiler.mods[0];
    //
    //     let (toks, _) = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
    //         .tokenize(&mut interner);
    //
    //     let ast_info = parser::parse(&settings, &module, &toks, &mut interner).0;
    //
    //     NamespaceResolver::new(
    //         &settings,
    //         &ast_info,
    //         &interner,
    //         module.mod_id,
    //         &mut compiler,
    //     )
    //     .resolve()
    //     .unwrap();
    //
    //     TypeResolver::new(
    //         &settings,
    //         &ast_info,
    //         Default::default(),
    //         &interner,
    //         &mut compiler,
    //     )
    //     .resolve()
    //     .unwrap();
    //
    //     ConstraintResolver::new(
    //         &settings,
    //         &[ast_info],
    //         &interner,
    //         Default::default(),
    //         &mut val_ctx,
    //         &mut compiler,
    //     )
    //     .resolve()
    //     .unwrap();
    //
    //     assert_eq!(compiler.symbols.len(), 1);
    //     assert_eq!(compiler.values.len() - pre_loaded_values, 1);
    //     match &compiler.values[compiler.values.len() - 1]
    //         .const_val
    //         .as_ref()
    //         .unwrap()
    //     {
    //         Value::InternedStr(_) => (),
    //         _ => panic!("Value mistmatch"),
    //     };
    //
    //     let text = "
    //             let CONSTANT = 0e-5
    //         ";
    //
    //     let (mut interner, settings, mut compiler) = mock_single_module_compiler(text);
    //
    //     let pre_loaded_values = compiler.values.len();
    //
    //     let module = &compiler.mods[0];
    //
    //     let (toks, _) = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
    //         .tokenize(&mut interner);
    //
    //     let ast_info = parser::parse(&settings, &module, &toks, &mut interner).0;
    //
    //     NamespaceResolver::new(
    //         &settings,
    //         &ast_info,
    //         &interner,
    //         module.mod_id,
    //         &mut compiler,
    //     )
    //     .resolve()
    //     .unwrap();
    //
    //     TypeResolver::new(
    //         &settings,
    //         &ast_info,
    //         Default::default(),
    //         &interner,
    //         &mut compiler,
    //     )
    //     .resolve()
    //     .unwrap();
    //
    //     ConstraintResolver::new(
    //         &settings,
    //         &[ast_info],
    //         &interner,
    //         Default::default(),
    //         &mut val_ctx,
    //         &mut compiler,
    //     )
    //     .resolve()
    //     .unwrap();
    //
    //     assert_eq!(compiler.symbols.len(), 1);
    //     assert_eq!(compiler.values.len() - pre_loaded_values, 1);
    //     match &compiler.values[compiler.values.len() - 1]
    //         .const_val
    //         .as_ref()
    //         .unwrap()
    //     {
    //         Value::F64(_) => (),
    //         _ => panic!("Value mistmatch"),
    //     };
    //
    //     let text = "
    //             let CONSTANT = true
    //         ";
    //
    //     let (mut interner, settings, mut compiler) = mock_single_module_compiler(text);
    //
    //     let module = &compiler.mods[0];
    //
    //     let (toks, _) = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
    //         .tokenize(&mut interner);
    //
    //     let ast_info = parser::parse(&settings, &module, &toks, &mut interner).0;
    //
    //     NamespaceResolver::new(
    //         &settings,
    //         &ast_info,
    //         &interner,
    //         module.mod_id,
    //         &mut compiler,
    //     )
    //     .resolve()
    //     .unwrap();
    //
    //     TypeResolver::new(
    //         &settings,
    //         &ast_info,
    //         Default::default(),
    //         &interner,
    //         &mut compiler,
    //     )
    //     .resolve()
    //     .unwrap();
    //
    //     ConstraintResolver::new(
    //         &settings,
    //         &[ast_info],
    //         &interner,
    //         Default::default(),
    //         &mut val_ctx,
    //         &mut compiler,
    //     )
    //     .resolve()
    //     .unwrap();
    //
    //     assert_eq!(compiler.symbols.len(), 1);
    //     assert_eq!(VALUE_TRUE_POS, 1);
    //     match &compiler.values[VALUE_TRUE_POS].const_val.as_ref().unwrap() {
    //         Value::Bool(true) => (),
    //         _ => panic!("Value mistmatch"),
    //     };
    //
    //     let text = "
    //             let CONSTANT = false
    //         ";
    //
    //     let (mut interner, settings, mut compiler) = mock_single_module_compiler(text);
    //
    //     let module = &compiler.mods[0];
    //
    //     let (toks, _) = Lexer::new(&module.metadata.src_bytes, module.metadata.script_start)
    //         .tokenize(&mut interner);
    //
    //     let ast_info = parser::parse(&settings, &module, &toks, &mut interner).0;
    //
    //     NamespaceResolver::new(
    //         &settings,
    //         &ast_info,
    //         &interner,
    //         module.mod_id,
    //         &mut compiler,
    //     )
    //     .resolve()
    //     .unwrap();
    //
    //     TypeResolver::new(
    //         &settings,
    //         &ast_info,
    //         Default::default(),
    //         &interner,
    //         &mut compiler,
    //     )
    //     .resolve()
    //     .unwrap();
    //
    //     ConstraintResolver::new(
    //         &settings,
    //         &[ast_info],
    //         &interner,
    //         Default::default(),
    //         &mut val_ctx,
    //         &mut compiler,
    //     )
    //     .resolve()
    //     .unwrap();
    //
    //     assert_eq!(compiler.symbols.len(), 1);
    //     assert_eq!(VALUE_FALSE_POS, 0);
    //     match &compiler.values[VALUE_FALSE_POS].const_val.as_ref().unwrap() {
    //         Value::Bool(false) => (),
    //         _ => panic!("Value mistmatch"),
    //     };
    // }
}
