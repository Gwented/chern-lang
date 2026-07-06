pub mod constraints;
pub mod cst;
pub mod lexer;
pub mod lookup;
pub mod modules;
pub mod parser;
pub mod resolvers;
pub mod script_compiler;
pub mod semantic;
pub mod user_defined;

#[cfg(test)]
mod tests {
    use crate::{
        lexer::token::{Notation, SpannedToken, Token, TokenKind},
        parser::ast::ast_concepts::AstInfo,
        resolvers::{constraint_resolver::ConstraintResolver, resolver_env::ResolverEnv},
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

    trait ConfigLoaderOutputExt {
        fn expect_success(self) -> SourceRegion;
    }

    impl ConfigLoaderOutputExt for ConfigLoaderOutput {
        fn expect_success(self) -> SourceRegion {
            match self {
                ConfigLoaderOutput::Success(region) => region,
                other => panic!("expected ConfigLoaderOutput::Success, got {other:?}"),
            }
        }
    }

    fn get_module_region<'a>(
        arena: &'a Arena<SourceRegion, SourceRegionId>,
        module: &Module,
    ) -> &'a SourceRegion {
        let region_id = module
            .region_id
            .expect("Module should have a source region");
        &arena[region_id]
    }

    fn mock_single_module_compiler(
        text: &str,
    ) -> (Arena<SourceRegion, SourceRegionId>, Intern, ChrnSettings, ScriptCompiler) {
        let interner = mock_interner(0, 1);
        let settings = ChrnSettings::default();
        let path_id = PathId::new(0);
        let region_id = SourceRegionId::new(0);

        let source_region =
            ConfigLoader::new(region_id, text.as_bytes(), path_id, &settings, &interner)
                .load_config()
                .expect_success();

        let module = Module::new(
            Default::default(),
            Default::default(),
            Default::default(),
            Default::default(),
            Some(region_id),
        );

        // Should use compiler store now
        let mut arena = Arena::<SourceRegion, SourceRegionId>::new();
        arena.push(source_region);
    let compiler = ScriptCompiler::init(None, Arena::<Module, ModuleId>::from(vec![module]));

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
            ConfigLoader::new(region_id, text.as_bytes(), path_id, &settings, interner)
                .load_config()
                .expect_success();

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
    ) -> (Arena<SourceRegion, SourceRegionId>, Intern, ChrnSettings, ScriptCompiler) {
        let interner = mock_interner(0, modules_with_regions.len());
        let settings = ChrnSettings::default();

        let (modules, regions): (Vec<Module>, Vec<SourceRegion>) =
            modules_with_regions.into_iter().unzip();

        let mut arena = Arena::<SourceRegion, SourceRegionId>::new();
        for region in regions {
            arena.push(region);
        }
    let compiler = ScriptCompiler::init(None, Arena::<Module, ModuleId>::from(modules));

    (arena, interner, settings, compiler)
}
    /// Builds resolver environments aligned with compiler modules from their ASTs
    fn build_resolver_envs<'a>(
        compiler: &ScriptCompiler,
        arena: &'a Arena<SourceRegion, SourceRegionId>,
        asts: &'a [Option<AstInfo>],
    ) -> Vec<Option<ResolverEnv<'a>>> {
        compiler
            .mods
            .iter()
            .enumerate()
            .map(|(i, module)| {
                module.region_id.map(|region_id| {
                    let region = &arena[region_id];
                    let ast = asts[i]
                        .as_ref()
                        .expect("Module with region_id should have an AstInfo entry");
                    ResolverEnv::new(ast, region, module.mod_id)
                })
            })
            .collect()
    }

    /// Runs member resolution, panicking on diagnostics
    fn run_member_resolver(
        settings: &ChrnSettings,
        envs: &[Option<ResolverEnv>],
        interner: &Intern,
        compiler: &mut ScriptCompiler,
    ) {
        let diags = MemberResolver::new(settings, envs, interner, compiler).resolve();
        assert!(diags.is_empty(), "Member resolution failed: {:?}", diags);
    }

    use std::path::Path;

    use chrn_utils::{
        arena::Arena,
        chrn_settings::ChrnSettings,
        core_error::ConfigLoadError,
        id_types::{InternedId, ModuleId, PathId, SourceRegionId, ValueId},
        intern::Intern,
        source_map::source_diagnostic::SourceDiagnostic,
        source_map::source_region::SourceRegion,
    };
    use lang::{
        config_loader::{ConfigLoader, ConfigLoaderOutput},
        keywords::Keyword,
        values::Value,
    };

    use crate::{
        lexer::Lexer,
        lookup::scopes::ScopeType,
        modules::{Import, ImportKind, Module, ModuleState},
        parser::{self},
        resolvers::{
            member_resolver::MemberResolver, name_resolver::NamespaceResolver,
            type_resolver::TypeResolver,
        },
        semantic::hir::hir_concepts::VariableState,
    };

    // -- Const dependency test helpers --

    /// Parses a single-module script and runs the full resolution pipeline up to and including
    /// constraints. Panics on any resolution error so that the returned compiler state is known to
    /// be fully resolved.
    fn compile_and_resolve_single_module(text: &str) -> (ScriptCompiler, Intern) {
        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);

        let module = &compiler.mods[ModuleId::new(0)];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        NamespaceResolver::new(&settings, &interner, &mut compiler)
            .resolve(&env)
            .unwrap();

        let env = ResolverEnv::new(&ast_info, region, compiler.mods[ModuleId::new(0)].mod_id);
        let envs = vec![Some(env)];
        run_member_resolver(&settings, &envs, &interner, &mut compiler);
        let env = envs[0].as_ref().expect("Env should exist");

        TypeResolver::new(&settings, &interner, &mut compiler)
            .resolve(env)
            .unwrap();
        ConstraintResolver::new(&settings, &interner, &mut compiler)
            .resolve(env)
            .unwrap();

        (compiler, interner)
    }

    /// Returns the constant value of a resolved `let` variable by name.
    fn value_of(compiler: &ScriptCompiler, interner: &Intern, name: &str) -> Value {
        let name_id = interner
            .try_search_str(name)
            .unwrap_or_else(|| panic!("Variable '{}' was not interned", name));
        let var_def = compiler
            .variables
            .iter()
            .find(|v| v.name_id == name_id)
            .unwrap_or_else(|| panic!("Variable '{}' not found", name));

        match &var_def.state {
            VariableState::Known(value_id) => compiler.values[*value_id]
                .const_val
                .clone()
                .unwrap_or_else(|| panic!("Variable '{}' has no constant value", name)),
            VariableState::ReservedTypeSlot(_) => {
                panic!("Variable '{}' is still a reserved type slot", name)
            }
        }
    }

    /// Runs namespace and member resolution, then returns the result of type resolution. This is
    /// useful for tests that want to assert that type resolution fails (e.g. circular const
    /// dependencies) without the constraint pass running.
    fn type_resolve_single_module(
        text: &str,
    ) -> Result<(ScriptCompiler, Intern), Vec<SourceDiagnostic>> {
        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);

        let module = &compiler.mods[ModuleId::new(0)];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        NamespaceResolver::new(&settings, &interner, &mut compiler)
            .resolve(&env)
            .unwrap();

        let env = ResolverEnv::new(&ast_info, region, compiler.mods[ModuleId::new(0)].mod_id);
        let envs = vec![Some(env)];
        run_member_resolver(&settings, &envs, &interner, &mut compiler);
        let env = envs[0].as_ref().expect("Env should exist");

        match TypeResolver::new(&settings, &interner, &mut compiler).resolve(env) {
            Ok(()) => Ok((compiler, interner)),
            Err(diags) => Err(diags),
        }
    }

    #[test]
    fn lex_tok_test() {
        let text = r#"bind "./some/path""#;

        let mut interner = Intern::init();
        let path_id = interner.intern_path(Path::new(""));
        let region_id = SourceRegionId::new(0);
        let metadata = ConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .expect_success();

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
        let opt = ConfigLoader::new(
            region_id,
            correct.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config();

        assert!(
            matches!(opt, ConfigLoaderOutput::Success(_)),
            "properly closed @def and @end should succeed"
        );

        // Improper @def without an @end
        // This type of error is more likely to break the diagnostic reporting but is fixed for
        // now.
        let wrong = r#"@defbind "./some/path""#;

        let opt = ConfigLoader::new(
            region_id,
            wrong.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config();

        assert!(
            matches!(opt, ConfigLoaderOutput::Broken(_, _)),
            "improper @def without @end should produce a Broken region"
        );
    }

    #[test]
    fn cfg_at_test() {
        // Properly closed @def and @end
        let content = "\n     @e\n";
        let mut interner = mock_interner(1, 1);
        let path_id = interner.intern_path(Path::new(""));
        let region_id = SourceRegionId::new(0);
        let settings = ChrnSettings::default();
        let region = ConfigLoader::new(
            region_id,
            content.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .expect_success();
        let region_str = str::from_utf8(&region.src_bytes[..]).unwrap();
        assert_eq!(region_str, content);

        let (toks, _) =
            Lexer::new(region_id, &region.src_bytes, region.script_start).tokenize(&mut interner);
        assert_eq!(toks.len(), 3);
        assert_eq!(toks[0].tok, Token::At);
        assert!(matches!(toks[1].tok, Token::Id(_)));
        assert_eq!(toks[2].tok, Token::EOF);

        let (_, diags) = parser::parse(&settings, &region, &toks, &interner);
        assert!(
            diags.len() > 0,
            "parser should have picked up at least one error"
        );
    }

    // -----------------------------------------------------------------------------------------
    // Config loader byte-consumption tests
    //
    // These tests target `ConfigLoader` directly with pathological input. They mirror the
    // spirit of `chrn_tests/other.chrn` which uses raw `@` text and odd whitespace, but at the
    // byte level. Each test exercises a specific corner of the byte-walking state machine so
    // that subtle off-by-one, lookahead, escape, or comment-handling regressions get caught.
    // -----------------------------------------------------------------------------------------

    /// Helper: runs the config loader on a raw byte slice and returns the resulting region.
    fn load_cfg_bytes(bytes: &[u8]) -> ConfigLoaderOutput {
        let mut interner = mock_interner(0, 1);
        let path_id = interner.intern_path(Path::new(""));
        let region_id = SourceRegionId::new(0);
        ConfigLoader::new(
            region_id,
            bytes,
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
    }

    /// Helper: runs the config loader on a string and returns the resulting region.
    fn load_cfg(text: &str) -> ConfigLoaderOutput {
        load_cfg_bytes(text.as_bytes())
    }

    /// `@def` immediately followed by `@end` with no separator. The loader does
    /// `self.skip(4)` then unconditionally `self.advance()` after the `@def` match, which
    /// consumes one extra byte and skips the `@` of `@end`. This test pins the current behavior
    /// so a future fix surfaces as a real test change rather than a silent regression.
    #[test]
    fn cfg_at_def_no_separator_before_at_end_test() {
        let res = load_cfg("@def@end");
        assert!(
            matches!(res, ConfigLoaderOutput::Success(_)),
            "Adjacent @def@end currently fails (off-by-one). Update this test if the loader \
             is fixed to detect the immediately-following @end."
        );
        let res = load_cfg(" @def@end ");
        assert!(matches!(res, ConfigLoaderOutput::Success(_)));
        let res = load_cfg("@def \t@end\n\t");
        assert!(matches!(res, ConfigLoaderOutput::Success(_)));
        let res = load_cfg(" @def @end");
        assert!(matches!(res, ConfigLoaderOutput::Success(_)));
        let res = load_cfg(" @def @end ");
        assert!(matches!(res, ConfigLoaderOutput::Success(_)));
        let res = load_cfg("@def\t@\re\rnd");
        assert!(matches!(res, ConfigLoaderOutput::Broken(_, _)));
    }

    /// `@end` (4 bytes) appearing with no preceding `@def` must NOT terminate a script
    /// block. The whole file is the script, and `@end` should be reported as plain text.
    #[test]
    fn cfg_at_end_without_at_def_is_plain_text_test() {
        let res = load_cfg("@end").expect_success();
        assert_eq!(res.src_bytes, b"@end");
        assert!(res.serial_start.is_none());
        assert_eq!(res.script_start, 0);
    }

    /// A NUL byte (`\0`) anywhere in the file should terminate the loader's scan
    /// immediately, regardless of whether an `@def` is in progress.
    #[test]
    fn cfg_null_byte_terminates_scan_mid_file_test() {
        // The bytes after the NUL are never observed, so the unclosed `@def` does NOT
        // produce a "missing @end" diagnostic - the NUL is treated as the end of the script.
        let res = load_cfg("@def var-> x: i32\0this would normally break things@end");
        assert!(
            matches!(res, ConfigLoaderOutput::Broken(_, _)),
            "NUL after @def should produce a Broken region, not silently swallow the missing-@end error"
        );
    }

    /// A NUL byte at the very start of the file should produce an empty region.
    #[test]
    fn cfg_null_byte_at_start_test() {
        let res = load_cfg("\0hello world").expect_success();
        dbg!(&res.src_bytes);
        assert_eq!(res.src_bytes, []);
        assert!(res.serial_start.is_none());
    }

    /// An `@` sign inside a double-quoted string must be treated as part of the string,
    /// NOT as a marker. The string is consumed by `read_quotes` before the `@` arm is reached.
    #[test]
    fn cfg_at_sign_inside_string_is_not_a_marker_test() {
        // The string contains "@def" as text. The loader should report no error and treat the
        // string as opaque content of the script body (no @def was ever seen at the top level).
        let res = load_cfg(r#""this has @def inside it" remaining"#).expect_success();
        assert!(res.serial_start.is_none(), "No @def was ever matched");
        let s = std::str::from_utf8(&res.src_bytes).unwrap();
        assert!(s.contains("@def inside it"));
    }

    /// The substring `/*` inside a string must NOT be treated as a multi-line comment.
    /// Confirms `read_quotes` fully consumes the string before any other branch fires.
    #[test]
    fn cfg_multi_comment_syntax_inside_string_is_not_comment_test() {
        let res = load_cfg(r#""/* still just text " trailing"#).expect_success();
        assert!(res.serial_start.is_none());
        assert!(
            std::str::from_utf8(&res.src_bytes)
                .unwrap()
                .contains("/* still just text ")
        );
    }

    /// `@def` written inside a `//` line comment must be ignored. The comment handler
    /// advances until `\n`, so the `@` arm never sees this `@def`.
    #[test]
    fn cfg_at_def_inside_line_comment_is_ignored_test() {
        let res = load_cfg("// @def @end\nreal code\n").expect_success();
        assert!(
            res.serial_start.is_none(),
            "@def inside a // comment must not open a block"
        );
        let s = std::str::from_utf8(&res.src_bytes).unwrap();
        assert!(s.contains("real code"));
    }

    /// `@def` written inside a `/* */` multi-line comment must be ignored. Tests the
    /// interaction between comment depth tracking and `@` matching.
    #[test]
    fn cfg_at_def_inside_multi_comment_is_ignored_test() {
        let res = load_cfg("/* @def @end */\nreal\n").expect_success();
        assert!(res.serial_start.is_none());

        let res = load_cfg("/*@def @end*/\nreal\n").expect_success();
        assert!(res.serial_start.is_none());

        let res = load_cfg("/*@def@end*/\r\nreal\n\x25").expect_success();
        assert!(res.serial_start.is_none());
    }

    /// A backslash escape inside a string must skip the next byte verbatim, so `"a\b"`
    /// closes at the second `"` and the `\b` is part of the string content. This catches
    /// off-by-one bugs in `read_quotes` where the escape could consume the closing quote.
    #[test]
    fn cfg_escape_sequence_in_string_test() {
        // Content: "a\b"  — the \b is an escape; the closing " is at index 4.
        let res = load_cfg(r#""a\b" after"#).expect_success();
        assert!(res.serial_start.is_none());
        let s = std::str::from_utf8(&res.src_bytes).unwrap();
        assert!(s.starts_with("\"a\\b\""));
    }

    /// A string opened with `"` and never closed must produce an unclosed-quotes error.
    /// The diagnostic should point to the opening quote location.
    #[test]
    fn cfg_unclosed_double_quote_errors_test() {
        let res = load_cfg("hello \"world");
        match res {
            ConfigLoaderOutput::Broken(_, ConfigLoadError::Diagnostic(_)) => {}
            other => panic!("Expected unclosed-quote error, got {other:?}"),
        }
    }

    /// A string opened with `'` and never closed must produce an unclosed-quotes error.
    /// The diagnostic should point to the opening quote location.
    #[test]
    fn cfg_unclosed_single_quote_errors_test() {
        let res = load_cfg("hello 'world");
        assert!(
            matches!(
                res,
                ConfigLoaderOutput::Broken(_, ConfigLoadError::Diagnostic(_))
            ),
            "unclosed single quotes should produce a Broken region with a Diagnostic"
        );
    }

    /// A backslash at the very end of the file, inside a string, must cause the string
    /// to be considered unclosed. The escape handler does `self.skip(2)`, so a trailing `\`
    /// runs off the buffer and `read_quotes` returns `Err`.
    #[test]
    fn cfg_escape_at_eof_in_string_errors_test() {
        let res = load_cfg(r#""abc\"#);
        assert!(
            matches!(
                res,
                ConfigLoaderOutput::Broken(_, ConfigLoadError::Diagnostic(_))
            ),
            "escape at EOF in string should produce a Broken region with a Diagnostic"
        );
    }

    /// An empty input should yield a valid empty region with no serial start and a
    /// script_start of 0. This is the canonical "no markers at all" case.
    #[test]
    fn cfg_empty_file_test() {
        let res = load_cfg("").expect_success();
        assert_eq!(res.src_bytes, []);
        assert_eq!(res.script_start, 0);
        assert!(res.serial_start.is_none());
    }

    /// `\r\n` (Windows) line endings must behave the same as `\n`. The line-comment
    /// handler stops at `\n`, but a stray `\r` should not cause issues. This catches any
    /// accidental `\n`-only termination.
    #[test]
    fn cfg_crlf_line_endings_test() {
        // Comment then real content with CRLF separators.
        let res = load_cfg("\r//\r\r\r header\r\nlet A = 1\r\nlet B = 2\r\n").expect_success();
        assert!(res.serial_start.is_none());
        let s = std::str::from_utf8(&res.src_bytes).unwrap();
        assert!(s.contains("let A = 1"));
        assert!(s.contains("let B = 2"));
    }

    /// A bare `@` at the end of the file with `requires_end == false` triggers the
    /// `!can_check` short-circuit branch which skips the remaining bytes and breaks. This
    /// must not panic and must report no error (no `@def` was opened).
    #[test]
    fn cfg_lone_at_sign_at_eof_test() {
        let res = load_cfg("some text @").expect_success();
        assert!(res.serial_start.is_none());
        let s = std::str::from_utf8(&res.src_bytes).unwrap();
        assert_eq!(s, "some text @");
    }

    /// A long run of `@` characters in normal text must all be consumed as individual
    /// `@` tokens, none of which form `@def` or `@end`. Verifies that the `@` arm's
    /// `self.advance()` covers the case where neither annotation matches.
    #[test]
    fn cfg_many_at_signs_in_a_row_test() {
        let res = load_cfg("@@@@@@@@@@@@ plain@ @ text @@@@@@@@@@@@").expect_success();
        assert!(res.serial_start.is_none());
        let s = std::str::from_utf8(&res.src_bytes).unwrap();
        dbg!(s);
        // Should be the entire file, untouched.
        assert_eq!(s, "@@@@@@@@@@@@ plain@ @ text @@@@@@@@@@@@");
    }

    /// A file containing only a multi-line comment that never closes must report an
    /// unclosed multi-line comment error. The handler tracks depth and produces a diagnostic
    /// pointing at the start of the comment.
    #[test]
    fn cfg_unclosed_multi_line_comment_test() {
        let res = load_cfg("/* this comment never ends");
        assert!(
            matches!(res, ConfigLoaderOutput::UnrecoverableErr(_)),
            "unclosed multi-line comment should produce an UnrecoverableErr"
        );
    }

    /// Tab characters must be treated as ordinary bytes by the loader — they are not
    /// treated as whitespace specially (the lexer would normalize later, but the loader
    /// must not skip or mis-handle them). This test interleaves tabs with `@def` and `@end`
    /// separated by tabs only to confirm the byte scanner does not confuse tab with newline.
    #[test]
    fn cfg_tab_characters_around_at_def_test() {
        // Use tabs (not spaces, not newlines) between @def and @end.
        // :crab:
        let res = load_cfg("\t@def\tva\nr->\tx:\ti3\r2\t\r\u{32}@end\t");
        match res {
            ConfigLoaderOutput::Success(region) => {
                // If the loader accepts it, the src_bytes should contain the whole input.
                let s = std::str::from_utf8(&region.src_bytes).unwrap();
                assert!(s.contains("@def"));
                assert!(s.contains("@end"));
                assert!(region.serial_start.is_some());
            }
            other => {
                // If the loader rejects it, the rejection should be about the actual
                // content (missing @end, etc.) — never a panic from a confused byte position.
                panic!("Loader errored on tab-separated @def/@end: {other:?}");
            }
        }
    }

    #[test]
    fn char_literal_test() {
        // Valid single character
        let text = "'a'";
        let mut interner = Intern::init();

        let path_id = interner.intern_path(Path::new(""));
        let region_id = SourceRegionId::new(0);

        let metadata = ConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .expect_success();
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

        let metadata = ConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .expect_success();

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

        let metadata = ConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .expect_success();

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
        let metadata = ConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .expect_success();
        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        assert!(
            matches!(toks[0].tok, Token::Invalid(_),),
            "Expected Invalid token, got {:?}",
            toks[0].tok
        );

        // Invalid hex escape
        let text = "'\\x2'";
        let mut interner = Intern::init();
        let path_id = interner.intern_path(Path::new(""));
        let region_id = SourceRegionId::new(0);
        let metadata = ConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .expect_success();
        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        assert!(
            matches!(toks[0].tok, Token::Invalid(_),),
            "Expected Invalid token, got {:?}",
            toks[0].tok
        );

        // I can't actually read hex
        // Invalid hex digits
        let text = "'\\x255'";
        let mut interner = Intern::init();
        let path_id = interner.intern_path(Path::new(""));
        let region_id = SourceRegionId::new(0);
        let metadata = ConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::new(),
            &interner,
        )
        .load_config()
        .expect_success();
        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        assert!(
            matches!(toks[0].tok, Token::Invalid(_),),
            "Expected Invalid token, got {:?}",
            toks[0].tok
        );

        // Unknown escape
        let text = "'\\q'";
        let mut interner = Intern::init();
        let path_id = interner.intern_path(Path::new(""));
        let region_id = SourceRegionId::new(0);
        let metadata = ConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::new(),
            &interner,
        )
        .load_config()
        .expect_success();
        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        assert!(
            matches!(toks[0].tok, Token::Invalid(_),),
            "Expected Invalid token, got {:?}",
            toks[0].tok
        );

        // Out of range escape
        let text = "'\\x1Y'";
        let mut interner = Intern::init();
        let path_id = interner.intern_path(Path::new(""));
        let region_id = SourceRegionId::new(0);
        let metadata = ConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::new(),
            &interner,
        )
        .load_config()
        .expect_success();
        let (toks, _) = Lexer::new(
            metadata.region_id,
            &metadata.src_bytes,
            metadata.script_start,
        )
        .tokenize(&mut interner);

        assert_eq!(2, toks.len());
        assert!(
            matches!(toks[0].tok, Token::Invalid(_),),
            "Expected Invalid token, got {:?}",
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

        let correct = ConfigLoader::new(
            region_id,
            correct,
            PathId::default(),
            &ChrnSettings::default(),
            &interner,
        )
        .load_config();

        let wrong = ConfigLoader::new(
            region_id,
            wrong,
            PathId::default(),
            &ChrnSettings::default(),
            &interner,
        )
        .load_config();

        assert!(matches!(correct, ConfigLoaderOutput::Success(_)));
        assert!(
            matches!(wrong, ConfigLoaderOutput::UnrecoverableErr(_)),
            "unclosed multi-line comment should produce an UnrecoverableErr"
        );
    }

    #[test]
    fn start_and_serial_offset_test() {
        let text = format!("adwh@def var-> int: i32 @endhi");
        let interner = mock_interner(0, 1);
        let region_id = SourceRegionId::new(0);

        let metadata = ConfigLoader::new(
            region_id,
            text.as_bytes(),
            PathId::default(),
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .expect_success();

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

        let metadata = ConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .expect_success();

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
        let metadata = ConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .expect_success();
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
        let metadata = ConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .expect_success();
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
        let metadata = ConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .expect_success();
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
        let metadata = ConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .expect_success();
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
        let metadata = ConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .expect_success();
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
        let metadata = ConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .expect_success();
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
        let metadata = ConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .expect_success();
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
        let metadata = ConfigLoader::new(
            region_id,
            text.as_bytes(),
            path_id,
            &ChrnSettings::default(),
            &interner,
        )
        .load_config()
        .expect_success();
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
    fn read_ident_includes_trailing_underscore() {
        let src: &[u8] = b"foo_";
        let mut interner = Intern::init();
        let mut lex = Lexer::new(SourceRegionId::new(0), src, 0);
        let (toks, _) = lex.tokenize(&mut interner);

        // Expect at least one identifier token: "foo_"
        let id_tok = toks
            .iter()
            .find_map(|st| match st.tok {
                Token::Id(id) => Some((id, st.span)),
                _ => None,
            })
            .expect("expected an Id token for \"foo_\"");

        let (id, span) = id_tok;
        let lexed = interner.search(id);
        assert_eq!(lexed, "foo_", "underscore should be included in ident");

        // Span must cover exactly the four bytes "foo_".
        assert_eq!(span.start, 0);
        assert_eq!(span.end, 4);
    }

    /// A bare `_` at the end of input must lex without panicking.
    #[test]
    fn read_ident_handles_bare_underscore() {
        let src: &[u8] = b"_";
        let mut interner = Intern::init();
        let mut lex = Lexer::new(SourceRegionId::new(0), src, 0);
        let (toks, _) = lex.tokenize(&mut interner);

        let id = toks
            .iter()
            .find_map(|st| match st.tok {
                Token::Id(id) => Some(id),
                _ => None,
            })
            .expect("expected an Id token for \"_\"");

        assert_eq!(interner.search(id), "_");
    }

    /// Identifiers that mix alphanumerics and underscores in various positions
    /// must all be lexed correctly.
    #[test]
    fn read_ident_mixed_alphanumeric_and_underscore() {
        // Separators between identifiers are tokens that don't contain
        // alphanumerics or underscores, so each Id token in the source
        // becomes one Id in the output.
        let src: &[u8] = b"foo_bar+_qux+a_b_c_";
        let mut interner = Intern::init();
        let mut lex = Lexer::new(SourceRegionId::new(0), src, 0);
        let (toks, _) = lex.tokenize(&mut interner);

        let names: Vec<String> = toks
            .iter()
            .filter_map(|st| match st.tok {
                Token::Id(id) => Some(interner.search(id).to_string()),
                _ => None,
            })
            .collect();

        assert_eq!(names, vec!["foo_bar", "_qux", "a_b_c_"]);
    }

    #[test]
    fn nameresolver_duplicate_simple_test() {
        // -- NEUTRAL --
        let wrong = "
                let DUPLICATE = 3
                let DUPLICATE = \"Hi\"
                ";

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(wrong);

        let module = &compiler.mods[ModuleId::new(0)];

        let region = get_module_region(&arena, module);
        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        let res = NamespaceResolver::new(&settings, &interner, &mut compiler).resolve(&env);

        assert_eq!(res.is_err(), true);

        let correct = "
                    let ORIGINAL = 2 + 2
                    let NEW = \"Hallo\"
                ";

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(correct);

        let module = &compiler.mods[ModuleId::new(0)];

        let region = get_module_region(&arena, module);
        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        let res = NamespaceResolver::new(&settings, &interner, &mut compiler).resolve(&env);

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

        let module = &compiler.mods[ModuleId::new(0)];

        let region = get_module_region(&arena, module);
        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        let res = NamespaceResolver::new(&settings, &interner, &mut compiler).resolve(&env);

        assert_eq!(res.is_err(), true);

        let correct = "
                var->
                    original: u32
                    new: i8
                ";

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(correct);

        let module = &compiler.mods[ModuleId::new(0)];

        let region = get_module_region(&arena, module);
        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        let res = NamespaceResolver::new(&settings, &interner, &mut compiler).resolve(&env);

        assert_eq!(res.is_ok(), true);

        // -- NEST --

        let wrong = "
                nest->
                    struct Duplicate {}
                    struct Duplicate {}
                ";

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(wrong);

        let module = &compiler.mods[ModuleId::new(0)];

        let region = get_module_region(&arena, module);
        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        let res = NamespaceResolver::new(&settings, &interner, &mut compiler).resolve(&env);

        assert_eq!(res.is_err(), true);

        let correct = "
                nest->
                    struct Original {}
                    struct New {}
                ";

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(correct);

        let module = &compiler.mods[ModuleId::new(0)];

        let region = get_module_region(&arena, module);
        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        let res = NamespaceResolver::new(&settings, &interner, &mut compiler).resolve(&env);

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
            Some(sub_region_id),
        );

        let mut region_arena = Arena::<SourceRegion, SourceRegionId>::new();
        region_arena.push(main_meta);
        region_arena.push(sub_meta);

        let mut compiler = ScriptCompiler::init(None, Arena::<Module, ModuleId>::from(vec![main_mod, sub_mod]));

        let mut asts: Vec<Option<AstInfo>> = Vec::new();

        for mod_idx in 0..compiler.mods.len() {
            let module = &compiler.mods[ModuleId::new(mod_idx)];
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

        let resolver_envs = build_resolver_envs(&compiler, &region_arena, &asts);

        {
            let mut ns_resolver = NamespaceResolver::new(&settings, &interner, &mut compiler);
            for env in resolver_envs.iter() {
                if let Some(env) = env {
                    ns_resolver.resolve(env).unwrap();
                }
            }
        }

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
            let module = &compiler.mods[ModuleId::new(mod_idx)];
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

        let resolver_envs = build_resolver_envs(&compiler, &arena, &asts);

        {
            let mut ns_resolver = NamespaceResolver::new(&settings, &interner, &mut compiler);
            for env in resolver_envs.iter() {
                if let Some(env) = env {
                    ns_resolver.resolve(env).unwrap();
                }
            }
        }

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
            let module = &compiler.mods[ModuleId::new(mod_idx)];
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

        let resolver_envs = build_resolver_envs(&compiler, &arena, &asts);

        {
            let mut ns_resolver = NamespaceResolver::new(&settings, &interner, &mut compiler);
            for env in resolver_envs.iter() {
                if let Some(env) = env {
                    ns_resolver.resolve(env).unwrap();
                }
            }
        }

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
            let module = &compiler.mods[ModuleId::new(mod_idx)];
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

        let resolver_envs = build_resolver_envs(&compiler, &arena, &asts);

        {
            let mut ns_resolver = NamespaceResolver::new(&settings, &interner, &mut compiler);
            for env in resolver_envs.iter() {
                if let Some(env) = env {
                    ns_resolver.resolve(env).unwrap();
                }
            }
        }

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
    fn scope_simple_test() {
        // -- NEUTRAL --
        let text = "
                let CONSTANT = 3
                ";

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);

        let module = &compiler.mods[ModuleId::new(0)];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        NamespaceResolver::new(&settings, &interner, &mut compiler)
            .resolve(&env)
            .unwrap();

        let module = &compiler.mods[ModuleId::new(0)];

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

        let module = &compiler.mods[ModuleId::new(0)];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        NamespaceResolver::new(&settings, &interner, &mut compiler)
            .resolve(&env)
            .unwrap();

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

        let module = &compiler.mods[ModuleId::new(0)];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        NamespaceResolver::new(&settings, &interner, &mut compiler)
            .resolve(&env)
            .unwrap();

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
        // let env = ResolverEnv::new(&ast_info, region, module.mod_id);
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
        // let env = ResolverEnv::new(&ast_info, region, module.mod_id);
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

        let module = &compiler.mods[ModuleId::new(0)];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        NamespaceResolver::new(&settings, &interner, &mut compiler)
            .resolve(&env)
            .unwrap();

        //TODO: Override and Complex
        let module = &compiler.mods[ModuleId::new(0)];
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

        let module = &compiler.mods[ModuleId::new(0)];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        NamespaceResolver::new(&settings, &interner, &mut compiler)
            .resolve(&env)
            .unwrap();

        let env = ResolverEnv::new(&ast_info, region, compiler.mods[ModuleId::new(0)].mod_id);
        let envs = vec![Some(env)];
        run_member_resolver(&settings, &envs, &interner, &mut compiler);
        let env = envs[0].as_ref().expect("Env should exist");
        let res = TypeResolver::new(&settings, &interner, &mut compiler).resolve(env);

        assert_eq!(res.is_err(), true);

        let correct = "
                var->
                    primitive: i32
                    declared_type: Thing
                nest->
                    struct Thing {}
                ";

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(correct);

        let module = &compiler.mods[ModuleId::new(0)];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        NamespaceResolver::new(&settings, &interner, &mut compiler)
            .resolve(&env)
            .unwrap();

        let env = ResolverEnv::new(&ast_info, region, compiler.mods[ModuleId::new(0)].mod_id);
        let envs = vec![Some(env)];
        run_member_resolver(&settings, &envs, &interner, &mut compiler);
        let env = envs[0].as_ref().expect("Env should exist");
        let res = TypeResolver::new(&settings, &interner, &mut compiler).resolve(env);

        assert_eq!(res.is_ok(), true);
    }

    #[test]
    fn type_resolver_complex_test() {
        let text = "
                let CONSTANT = 4
                ";

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);

        let module = &compiler.mods[ModuleId::new(0)];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        NamespaceResolver::new(&settings, &interner, &mut compiler)
            .resolve(&env)
            .unwrap();

        let env = ResolverEnv::new(&ast_info, region, Default::default());
        let envs = vec![Some(env)];
        run_member_resolver(&settings, &envs, &interner, &mut compiler);
        let env = envs[0].as_ref().expect("Env should exist");
        TypeResolver::new(&settings, &interner, &mut compiler)
            .resolve(env)
            .unwrap();

        assert_eq!(compiler.values.len(), 1);
    }

    #[test]
    fn variable_declaration_test() {
        // let CONSTANT = 4
        let text = "
                let CONSTANT = 4
                ";

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);

        let module = &compiler.mods[ModuleId::new(0)];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        NamespaceResolver::new(&settings, &interner, &mut compiler)
            .resolve(&env)
            .unwrap();

        let env = ResolverEnv::new(&ast_info, region, compiler.mods[ModuleId::new(0)].mod_id);
        let envs = vec![Some(env)];
        run_member_resolver(&settings, &envs, &interner, &mut compiler);
        let env = envs[0].as_ref().expect("Env should exist");

        TypeResolver::new(&settings, &interner, &mut compiler)
            .resolve(env)
            .unwrap();

        ConstraintResolver::new(&settings, &interner, &mut compiler)
            .resolve(env)
            .unwrap();

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

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);

        let module = &compiler.mods[ModuleId::new(0)];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        NamespaceResolver::new(&settings, &interner, &mut compiler)
            .resolve(&env)
            .unwrap();

        let env = ResolverEnv::new(&ast_info, region, compiler.mods[ModuleId::new(0)].mod_id);
        let envs = vec![Some(env)];
        run_member_resolver(&settings, &envs, &interner, &mut compiler);
        let env = envs[0].as_ref().expect("Env should exist");

        TypeResolver::new(&settings, &interner, &mut compiler)
            .resolve(env)
            .unwrap();

        ConstraintResolver::new(&settings, &interner, &mut compiler)
            .resolve(env)
            .unwrap();

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

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);

        let module = &compiler.mods[ModuleId::new(0)];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        NamespaceResolver::new(&settings, &interner, &mut compiler)
            .resolve(&env)
            .unwrap();

        let env = ResolverEnv::new(&ast_info, region, compiler.mods[ModuleId::new(0)].mod_id);
        let envs = vec![Some(env)];
        run_member_resolver(&settings, &envs, &interner, &mut compiler);
        let env = envs[0].as_ref().expect("Env should exist");

        TypeResolver::new(&settings, &interner, &mut compiler)
            .resolve(env)
            .unwrap();

        ConstraintResolver::new(&settings, &interner, &mut compiler)
            .resolve(env)
            .unwrap();

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

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);

        let module = &compiler.mods[ModuleId::new(0)];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        NamespaceResolver::new(&settings, &interner, &mut compiler)
            .resolve(&env)
            .unwrap();

        let env = ResolverEnv::new(&ast_info, region, compiler.mods[ModuleId::new(0)].mod_id);
        let envs = vec![Some(env)];
        run_member_resolver(&settings, &envs, &interner, &mut compiler);
        let env = envs[0].as_ref().expect("Env should exist");

        TypeResolver::new(&settings, &interner, &mut compiler)
            .resolve(env)
            .unwrap();

        ConstraintResolver::new(&settings, &interner, &mut compiler)
            .resolve(env)
            .unwrap();

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

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);

        let module = &compiler.mods[ModuleId::new(0)];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        NamespaceResolver::new(&settings, &interner, &mut compiler)
            .resolve(&env)
            .unwrap();

        let env = ResolverEnv::new(&ast_info, region, compiler.mods[ModuleId::new(0)].mod_id);
        let envs = vec![Some(env)];
        run_member_resolver(&settings, &envs, &interner, &mut compiler);
        let env = envs[0].as_ref().expect("Env should exist");

        TypeResolver::new(&settings, &interner, &mut compiler)
            .resolve(env)
            .unwrap();

        ConstraintResolver::new(&settings, &interner, &mut compiler)
            .resolve(env)
            .unwrap();

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

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);

        let module = &compiler.mods[ModuleId::new(0)];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        NamespaceResolver::new(&settings, &interner, &mut compiler)
            .resolve(&env)
            .unwrap();

        let env = ResolverEnv::new(&ast_info, region, compiler.mods[ModuleId::new(0)].mod_id);
        let envs = vec![Some(env)];
        run_member_resolver(&settings, &envs, &interner, &mut compiler);
        let env = envs[0].as_ref().expect("Env should exist");

        TypeResolver::new(&settings, &interner, &mut compiler)
            .resolve(env)
            .unwrap();

        ConstraintResolver::new(&settings, &interner, &mut compiler)
            .resolve(env)
            .unwrap();

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

        let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);

        let module = &compiler.mods[ModuleId::new(0)];
        let region = get_module_region(&arena, module);

        let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
            .tokenize(&mut interner);

        let ast_info = parser::parse(&settings, region, &toks, &interner).0;

        let env = ResolverEnv::new(&ast_info, region, module.mod_id);
        NamespaceResolver::new(&settings, &interner, &mut compiler)
            .resolve(&env)
            .unwrap();

        let env = ResolverEnv::new(&ast_info, region, compiler.mods[ModuleId::new(0)].mod_id);
        let envs = vec![Some(env)];
        run_member_resolver(&settings, &envs, &interner, &mut compiler);
        let env = envs[0].as_ref().expect("Env should exist");

        TypeResolver::new(&settings, &interner, &mut compiler)
            .resolve(env)
            .unwrap();

        ConstraintResolver::new(&settings, &interner, &mut compiler)
            .resolve(env)
            .unwrap();

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
            let (arena, mut interner, settings, mut compiler) = mock_single_module_compiler(text);
            let module = &compiler.mods[ModuleId::new(0)];
            let region = get_module_region(&arena, module);
            let (toks, _) = Lexer::new(region.region_id, &region.src_bytes, region.script_start)
                .tokenize(&mut interner);
            let ast_info = parser::parse(&settings, region, &toks, &interner).0;
            let env = ResolverEnv::new(&ast_info, region, module.mod_id);
            NamespaceResolver::new(&settings, &interner, &mut compiler)
                .resolve(&env)
                .unwrap();
            let env = ResolverEnv::new(&ast_info, region, compiler.mods[ModuleId::new(0)].mod_id);
            let envs = vec![Some(env)];
            run_member_resolver(&settings, &envs, &interner, &mut compiler);
            let env = envs[0].as_ref().expect("Env should exist");
            TypeResolver::new(&settings, &interner, &mut compiler)
                .resolve(env)
                .unwrap();
            ConstraintResolver::new(&settings, &interner, &mut compiler)
                .resolve(env)
                .unwrap();
            let name_id = interner.try_search_str("X").unwrap();
            let var_def = compiler
                .variables
                .iter()
                .find(|v| v.name_id == name_id)
                .expect("Variable 'X' not found");
            match &var_def.state {
                VariableState::Known(value_id) => compiler.values[*value_id]
                    .const_val
                    .clone()
                    .expect("Variable 'X' has no const_val"),
                _ => panic!("Variable 'X' is not resolved"),
            }
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
        // Linear dependency cycle should be rejected
        assert!(
            type_resolve_single_module("let x = y\nlet y = x").is_err(),
            "Two-variable cycle should be rejected"
        );

        // Direct self reference.
        assert!(
            type_resolve_single_module("let X = X").is_err(),
            "Self-referencing constant should be rejected"
        );

        // Three-variable cycle.
        assert!(
            type_resolve_single_module("let A = B + 1\nlet B = C * 2\nlet C = A").is_err(),
            "Three-variable cycle should be rejected"
        );

        // Long indirect cycle.
        assert!(
            type_resolve_single_module(
                "
                    let A = B
                    let B = C
                    let C = D
                    let D = E
                    let E = A
                ",
            )
            .is_err(),
            "Long indirect cycle should be rejected"
        );

        // Cycle hidden inside a larger expression.
        assert!(
            type_resolve_single_module("let X = (Y + 2) * 3\nlet Y = X - 1").is_err(),
            "Cycle inside a complex expression should be rejected"
        );

        // Multiple independent cycles in the same module.
        assert!(
            type_resolve_single_module(
                "
                    let A = B
                    let B = A
                    let C = D + 1
                    let D = C
                ",
            )
            .is_err(),
            "Multiple independent cycles should be rejected"
        );

        // A chain that leads into a cycle.
        assert!(
            type_resolve_single_module(
                "
                    let A = B + 1
                    let B = C
                    let C = B
                ",
            )
            .is_err(),
            "Chain leading into a cycle should be rejected"
        );
    }

    // -- Cross-module expression dependency tests --

    /// Parses and fully resolves a two-module system where the main module imports the sub
    /// module (no alias). Panics on any resolution error.
    fn compile_and_resolve_cross_module(
        main_text: &str,
        sub_text: &str,
    ) -> (ScriptCompiler, Intern) {
        let mut interner = Intern::init();

        let import = mock_import(
            "sub_module",
            "sub_path",
            ModuleId::new(1),
            None,
            &mut interner,
        );

        let (main_mod, main_region) = mock_single_module(
            "main",
            "main_path",
            vec![import],
            0,
            main_text,
            &mut interner,
        );

        let (sub_mod, sub_region) = mock_single_module(
            "sub_module",
            "sub_path",
            Default::default(),
            1,
            sub_text,
            &mut interner,
        );

        let (arena, _, settings, mut compiler) =
            mock_multiple_module_compiler(vec![(main_mod, main_region), (sub_mod, sub_region)]);

        let mut asts: Vec<Option<AstInfo>> = Vec::new();
        for mod_idx in 0..compiler.mods.len() {
            let module = &compiler.mods[ModuleId::new(mod_idx)];
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

        let resolver_envs = build_resolver_envs(&compiler, &arena, &asts);

        let mut ns_resolver = NamespaceResolver::new(&settings, &interner, &mut compiler);
        for env in resolver_envs.iter() {
            if let Some(env) = env {
                ns_resolver.resolve(env).unwrap();
            }
        }

        run_member_resolver(&settings, &resolver_envs, &interner, &mut compiler);

        let mut ty_resolver = TypeResolver::new(&settings, &interner, &mut compiler);
        for env in resolver_envs.iter() {
            if let Some(env) = env {
                ty_resolver.resolve(env).unwrap();
            }
        }

        for env in resolver_envs.iter().flatten() {
            ConstraintResolver::new(&settings, &interner, &mut compiler)
                .resolve(env)
                .unwrap();
        }

        (compiler, interner)
    }

    /// Runs namespace and member resolution across two modules, then type resolution.
    /// Returns Ok if all type resolutions pass, Err with all diagnostics otherwise.
    fn type_resolve_cross_module(
        main_text: &str,
        sub_text: &str,
    ) -> Result<(ScriptCompiler, Intern), Vec<SourceDiagnostic>> {
        let mut interner = Intern::init();

        let import = mock_import(
            "sub_module",
            "sub_path",
            ModuleId::new(1),
            None,
            &mut interner,
        );

        let (main_mod, main_region) = mock_single_module(
            "main",
            "main_path",
            vec![import],
            0,
            main_text,
            &mut interner,
        );

        let (sub_mod, sub_region) = mock_single_module(
            "sub_module",
            "sub_path",
            Default::default(),
            1,
            sub_text,
            &mut interner,
        );

        let (arena, _, settings, mut compiler) =
            mock_multiple_module_compiler(vec![(main_mod, main_region), (sub_mod, sub_region)]);

        let mut asts: Vec<Option<AstInfo>> = Vec::new();
        for mod_idx in 0..compiler.mods.len() {
            let module = &compiler.mods[ModuleId::new(mod_idx)];
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

        let resolver_envs = build_resolver_envs(&compiler, &arena, &asts);

        {
            let mut ns_resolver = NamespaceResolver::new(&settings, &interner, &mut compiler);
            for env in resolver_envs.iter() {
                if let Some(env) = env {
                    ns_resolver.resolve(env).unwrap();
                }
            }
        }

        run_member_resolver(&settings, &resolver_envs, &interner, &mut compiler);

        let mut all_diags = Vec::new();

        let mut ty_resolver = TypeResolver::new(&settings, &interner, &mut compiler);
        for env in resolver_envs.iter() {
            if let Some(env) = env {
                if let Err(diags) = ty_resolver.resolve(env) {
                    all_diags.extend(diags);
                }
            }
        }

        if all_diags.is_empty() {
            Ok((compiler, interner))
        } else {
            Err(all_diags)
        }
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
            let module = &compiler.mods[ModuleId::new(mod_idx)];
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

        let resolver_envs = build_resolver_envs(&compiler, &arena, &asts);

        let mut ns_resolver = NamespaceResolver::new(&settings, &interner, &mut compiler);
        for env in resolver_envs.iter() {
            if let Some(env) = env {
                ns_resolver.resolve(env).unwrap();
            }
        }

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
            let module = &compiler.mods[ModuleId::new(mod_idx)];
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

        let resolver_envs = build_resolver_envs(&compiler, &arena, &asts);

        let mut ns_resolver = NamespaceResolver::new(&settings, &interner, &mut compiler);
        for env in resolver_envs.iter() {
            if let Some(env) = env {
                ns_resolver.resolve(env).unwrap();
            }
        }

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
}
