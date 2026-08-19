use crate::config_loader::{ConfigLoader, ConfigLoaderOutput};

use super::helpers::*;

#[test]
fn lex_tok_test() {
    let text = r#"bind "./some/path""#;

    let mut interner = Intern::init();
    let path_id = interner.intern_path(Path::new(""));
    let region_id = SourceRegionId::new(0);
    let metadata = ConfigLoader::new(region_id, text.as_bytes(), path_id, &ChrnConfig::default())
        .load_config()
        .expect_success();

    let toks = Lexer::new(
        metadata.region_id,
        &metadata.src_bytes,
        metadata.script_start,
        &mut ChrnConfig::default(),
    )
    .tokenize(&mut interner)
    .toks;

    assert_eq!(
        None, metadata.serial_start,
        "start_offset without `@def` failed"
    );
    assert_eq!(3, toks.len(), "expected exactly 3 tokens");

    // Keyword("bind") spanning bytes 0..4
    assert!(
        matches!(toks[0].tok, Token::Keyword(Keyword::Bind)),
        "expected Keyword(Bind) token"
    );
    assert_eq!(toks[0].span.start, 0);
    assert_eq!(toks[0].span.end, 4);

    // Str("./some/path") spanning bytes 5..18 (includes both quotes)
    assert!(matches!(toks[1].tok, Token::Str(_)), "expected Str token");
    assert_eq!(toks[1].span.start, 5);
    assert_eq!(toks[1].span.end, 18);

    assert_eq!(toks[2].tok, Token::EOF);
    assert_eq!(toks[2].span.start, 17);
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
        &ChrnConfig::default(),
    )
    .load_config();

    let region = match opt {
        ConfigLoaderOutput::Success(region, _) => region,
        other => panic!("properly closed @def and @end should succeed, got {other:?}"),
    };

    assert_eq!(region.script_start, 0);
    assert_eq!(region.serial_start, Some(26));

    let toks = Lexer::new(
        region.region_id,
        &region.src_bytes,
        region.script_start,
        &mut ChrnConfig::default(),
    )
    .tokenize(&mut interner)
    .toks;

    // Expect: Def(0,4), Id("bind")(4,8), Str("./some/path")(9,22), End(22,26)
    assert_eq!(toks.len(), 4);
    assert_eq!(toks[0].tok, Token::Def);
    assert_eq!(toks[0].span.start, 0);
    assert_eq!(toks[0].span.end, 4);
    assert!(
        matches!(toks[1].tok, Token::Keyword(Keyword::Bind)),
        "expected Keyword(Bind) after @def"
    );
    assert_eq!(toks[1].span.start, 4);
    assert_eq!(toks[1].span.end, 8);
    assert!(matches!(toks[2].tok, Token::Str(_)));
    assert_eq!(toks[2].span.start, 9);
    assert_eq!(toks[2].span.end, 22);
    assert_eq!(toks[3].tok, Token::End);
    assert_eq!(toks[3].span.start, 22);
    assert_eq!(toks[3].span.end, 26);

    // Improper @def without an @end
    // This type of error is more likely to break the diagnostic reporting but is fixed for
    // now.
    let wrong = r#"@defbind "./some/path""#;

    let opt = ConfigLoader::new(region_id, wrong.as_bytes(), path_id, &ChrnConfig::default())
        .load_config();

    match opt {
        ConfigLoaderOutput::Broken(region, _) => {
            assert_eq!(region.script_start, 0, "@def at offset 0");
            assert!(region.serial_start.is_none(), "no @end found");
        }
        other => panic!("improper @def without @end should produce a Broken region, got {other:?}"),
    }
}

#[test]
fn cfg_at_test() {
    // Properly closed @def and @end
    let content = "\n     @e\n";
    let mut interner = mock_interner(1, 1);
    let path_id = interner.intern_path(Path::new(""));
    let region_id = SourceRegionId::new(0);
    let mut settings = ChrnConfig::default();
    let region = ConfigLoader::new(
        region_id,
        content.as_bytes(),
        path_id,
        &ChrnConfig::default(),
    )
    .load_config()
    .expect_success();
    let region_str = str::from_utf8(&region.src_bytes[..]).unwrap();
    assert_eq!(region_str, content);

    let toks = Lexer::new(
        region_id,
        &region.src_bytes,
        region.script_start,
        &mut settings,
    )
    .tokenize(&mut interner)
    .toks;
    assert_eq!(toks.len(), 3);
    assert_eq!(toks[0].tok, Token::At);
    assert_eq!(toks[0].span.start, 6);
    assert_eq!(toks[0].span.end, 7);
    assert!(matches!(toks[1].tok, Token::Id(_)));
    assert_eq!(toks[1].span.start, 7);
    assert_eq!(toks[1].span.end, 8);
    assert_eq!(toks[2].tok, Token::EOF);
    assert_eq!(toks[2].span.start, 8);

    let (_, diags) = parser::parse(&mut settings, &region, &toks, &interner);
    assert!(
        !diags.diags.is_empty(),
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
#[test]
fn char_literal_test() {
    // Valid single character
    let text = "'a'";
    let mut interner = Intern::init();

    let path_id = interner.intern_path(Path::new(""));
    let region_id = SourceRegionId::new(0);

    let metadata = ConfigLoader::new(region_id, text.as_bytes(), path_id, &ChrnConfig::default())
        .load_config()
        .expect_success();
    let toks = Lexer::new(
        metadata.region_id,
        &metadata.src_bytes,
        metadata.script_start,
        &mut ChrnConfig::default(),
    )
    .tokenize(&mut interner)
    .toks;

    assert_eq!(2, toks.len());
    assert!(
        matches!(toks[0].tok, Token::Char(_),),
        "Expected char token, got {:?}",
        toks[0].tok
    );
    // `'a'` = 3 bytes: span (0, 3)
    assert_eq!(toks[0].span.start, 0);
    assert_eq!(toks[0].span.end, 3);
    assert_eq!(toks[1].tok, Token::EOF);
    assert_eq!(toks[1].span.start, 2);

    // Valid escaped character
    let text = "'\\n'";
    let mut interner = Intern::init();

    let path_id = interner.intern_path(Path::new(""));
    let region_id = SourceRegionId::new(0);

    let metadata = ConfigLoader::new(region_id, text.as_bytes(), path_id, &ChrnConfig::default())
        .load_config()
        .expect_success();

    let toks = Lexer::new(
        metadata.region_id,
        &metadata.src_bytes,
        metadata.script_start,
        &mut ChrnConfig::default(),
    )
    .tokenize(&mut interner)
    .toks;

    assert_eq!(2, toks.len());
    assert!(
        matches!(toks[0].tok, Token::Char(_),),
        "Expected char token, got {:?}",
        toks[0].tok
    );
    // `'\n'` = 4 bytes: span (0, 4)
    assert_eq!(toks[0].span.start, 0);
    assert_eq!(toks[0].span.end, 4);
    assert_eq!(toks[1].tok, Token::EOF);
    assert_eq!(toks[1].span.start, 3);

    // Valid hex escape
    let text = "'\\x2F'";

    let mut interner = Intern::init();
    let path_id = interner.intern_path(Path::new(""));
    let region_id = SourceRegionId::new(0);

    let metadata = ConfigLoader::new(region_id, text.as_bytes(), path_id, &ChrnConfig::default())
        .load_config()
        .expect_success();

    let toks = Lexer::new(
        metadata.region_id,
        &metadata.src_bytes,
        metadata.script_start,
        &mut ChrnConfig::default(),
    )
    .tokenize(&mut interner)
    .toks;

    assert_eq!(2, toks.len());
    assert!(
        matches!(toks[0].tok, Token::Char(_),),
        "Expected char token, got {:?}",
        toks[0].tok
    );
    // `'\x2F'` = 6 bytes: span (0, 6)
    assert_eq!(toks[0].span.start, 0);
    assert_eq!(toks[0].span.end, 6);
    assert_eq!(toks[1].tok, Token::EOF);
    assert_eq!(toks[1].span.start, 5);

    // Invalid character
    let text = "'aa'";
    let mut interner = Intern::init();
    let path_id = interner.intern_path(Path::new(""));
    let region_id = SourceRegionId::new(0);
    let metadata = ConfigLoader::new(region_id, text.as_bytes(), path_id, &ChrnConfig::default())
        .load_config()
        .expect_success();
    let toks = Lexer::new(
        metadata.region_id,
        &metadata.src_bytes,
        metadata.script_start,
        &mut ChrnConfig::default(),
    )
    .tokenize(&mut interner)
    .toks;

    assert_eq!(2, toks.len());
    assert!(
        matches!(toks[0].tok, Token::Invalid(_),),
        "Expected Invalid token, got {:?}",
        toks[0].tok
    );
    // `'aa'` = 4 bytes: span (0, 4)
    assert_eq!(toks[0].span.start, 0);
    assert_eq!(toks[0].span.end, 4);
    assert_eq!(toks[1].tok, Token::EOF);
    assert_eq!(toks[1].span.start, 3);

    // Invalid hex escape
    let text = "'\\x2'";
    let mut interner = Intern::init();
    let path_id = interner.intern_path(Path::new(""));
    let region_id = SourceRegionId::new(0);
    let metadata = ConfigLoader::new(region_id, text.as_bytes(), path_id, &ChrnConfig::default())
        .load_config()
        .expect_success();
    let toks = Lexer::new(
        metadata.region_id,
        &metadata.src_bytes,
        metadata.script_start,
        &mut ChrnConfig::default(),
    )
    .tokenize(&mut interner)
    .toks;

    assert_eq!(2, toks.len());
    assert!(
        matches!(toks[0].tok, Token::Invalid(_),),
        "Expected Invalid token, got {:?}",
        toks[0].tok
    );
    // `'\x2'` = 5 bytes: span (0, 5)
    assert_eq!(toks[0].span.start, 0);
    assert_eq!(toks[0].span.end, 5);
    assert_eq!(toks[1].tok, Token::EOF);
    assert_eq!(toks[1].span.start, 4);

    // I can't actually read hex
    // Invalid hex digits
    let text = "'\\x255'";
    let mut interner = Intern::init();
    let path_id = interner.intern_path(Path::new(""));
    let region_id = SourceRegionId::new(0);
    let metadata = ConfigLoader::new(region_id, text.as_bytes(), path_id, &ChrnConfig::default())
        .load_config()
        .expect_success();
    let toks = Lexer::new(
        metadata.region_id,
        &metadata.src_bytes,
        metadata.script_start,
        &mut ChrnConfig::default(),
    )
    .tokenize(&mut interner)
    .toks;

    assert_eq!(2, toks.len());
    assert!(
        matches!(toks[0].tok, Token::Invalid(_),),
        "Expected Invalid token, got {:?}",
        toks[0].tok
    );
    // `'\x255'` = 7 bytes: span (0, 7)
    assert_eq!(toks[0].span.start, 0);
    assert_eq!(toks[0].span.end, 7);
    assert_eq!(toks[1].tok, Token::EOF);
    assert_eq!(toks[1].span.start, 6);

    // Unknown escape
    let text = "'\\q'";
    let mut interner = Intern::init();
    let path_id = interner.intern_path(Path::new(""));
    let region_id = SourceRegionId::new(0);
    let metadata = ConfigLoader::new(region_id, text.as_bytes(), path_id, &ChrnConfig::default())
        .load_config()
        .expect_success();
    let toks = Lexer::new(
        metadata.region_id,
        &metadata.src_bytes,
        metadata.script_start,
        &mut ChrnConfig::default(),
    )
    .tokenize(&mut interner)
    .toks;

    assert_eq!(2, toks.len());
    assert!(
        matches!(toks[0].tok, Token::Invalid(_),),
        "Expected Invalid token, got {:?}",
        toks[0].tok
    );
    // `'\q'` = 4 bytes: span (0, 4)
    assert_eq!(toks[0].span.start, 0);
    assert_eq!(toks[0].span.end, 4);
    assert_eq!(toks[1].tok, Token::EOF);
    assert_eq!(toks[1].span.start, 3);

    // Out of range escape
    let text = "'\\x1Y'";
    let mut interner = Intern::init();
    let path_id = interner.intern_path(Path::new(""));
    let region_id = SourceRegionId::new(0);
    let metadata = ConfigLoader::new(region_id, text.as_bytes(), path_id, &ChrnConfig::default())
        .load_config()
        .expect_success();
    let toks = Lexer::new(
        metadata.region_id,
        &metadata.src_bytes,
        metadata.script_start,
        &mut ChrnConfig::default(),
    )
    .tokenize(&mut interner)
    .toks;

    assert_eq!(2, toks.len());
    assert!(
        matches!(toks[0].tok, Token::Invalid(_),),
        "Expected Invalid token, got {:?}",
        toks[0].tok
    );
    // `'\x1Y'` = 6 bytes: span (0, 6)
    assert_eq!(toks[0].span.start, 0);
    assert_eq!(toks[0].span.end, 6);
    assert_eq!(toks[1].tok, Token::EOF);
    assert_eq!(toks[1].span.start, 5);
}

#[test]
fn lex_notation_test() {
    // Hex Test (Hex Text (Hex Test))
    let text = "0xff";
    let mut interner = mock_interner(1, 1);

    let path_id = PathId::new(0);
    let region_id = SourceRegionId::new(0);

    let metadata = ConfigLoader::new(region_id, text.as_bytes(), path_id, &ChrnConfig::default())
        .load_config()
        .expect_success();

    let toks = Lexer::new(
        metadata.region_id,
        &metadata.src_bytes,
        metadata.script_start,
        &mut ChrnConfig::default(),
    )
    .tokenize(&mut interner)
    .toks;

    assert_eq!(2, toks.len());
    match toks[0].tok {
        Token::Integer(id, Notation::Hex) => {
            assert_eq!("255", interner.search(id));
        }
        _ => panic!("Expected Integer with Hex, found {:?}", toks[0].tok),
    }
    assert_eq!(toks[0].span.start, 0);
    assert_eq!(toks[0].span.end, 4);
    assert_eq!(toks[1].tok, Token::EOF);
    assert_eq!(toks[1].span.start, 3);

    // Binary
    let text = "0b1010";
    let metadata = ConfigLoader::new(region_id, text.as_bytes(), path_id, &ChrnConfig::default())
        .load_config()
        .expect_success();
    let toks = Lexer::new(
        metadata.region_id,
        &metadata.src_bytes,
        metadata.script_start,
        &mut ChrnConfig::default(),
    )
    .tokenize(&mut interner)
    .toks;

    assert_eq!(2, toks.len());
    match toks[0].tok {
        Token::Integer(id, Notation::Bin) => {
            assert_eq!("10", interner.search(id));
        }
        _ => panic!("Expected Integer with Binary, found {:?}", toks[0].tok),
    }
    assert_eq!(toks[0].span.start, 0);
    assert_eq!(toks[0].span.end, 6);
    assert_eq!(toks[1].tok, Token::EOF);
    assert_eq!(toks[1].span.start, 5);

    // Octal
    let text = "0o77";
    let metadata = ConfigLoader::new(region_id, text.as_bytes(), path_id, &ChrnConfig::default())
        .load_config()
        .expect_success();
    let toks = Lexer::new(
        metadata.region_id,
        &metadata.src_bytes,
        metadata.script_start,
        &mut ChrnConfig::default(),
    )
    .tokenize(&mut interner)
    .toks;

    assert_eq!(2, toks.len());
    match toks[0].tok {
        Token::Integer(id, Notation::Octal) => {
            assert_eq!("63", interner.search(id));
        }
        _ => panic!("Expected Integer with Octal, found {:?}", toks[0].tok),
    }
    assert_eq!(toks[0].span.start, 0);
    assert_eq!(toks[0].span.end, 4);
    assert_eq!(toks[1].tok, Token::EOF);
    assert_eq!(toks[1].span.start, 3);

    // Decimal
    let text = "42";
    let metadata = ConfigLoader::new(region_id, text.as_bytes(), path_id, &ChrnConfig::default())
        .load_config()
        .expect_success();
    let toks = Lexer::new(
        metadata.region_id,
        &metadata.src_bytes,
        metadata.script_start,
        &mut ChrnConfig::default(),
    )
    .tokenize(&mut interner)
    .toks;

    assert_eq!(2, toks.len());
    match toks[0].tok {
        Token::Integer(id, Notation::Decimal) => {
            assert_eq!("42", interner.search(id));
        }
        _ => panic!("Expected Integer of Decimal, found {:?}", toks[0].tok),
    }
    assert_eq!(toks[0].span.start, 0);
    assert_eq!(toks[0].span.end, 2);
    assert_eq!(toks[1].tok, Token::EOF);
    assert_eq!(toks[1].span.start, 1);

    // Float with decimal
    let text = "3.14";
    let metadata = ConfigLoader::new(region_id, text.as_bytes(), path_id, &ChrnConfig::default())
        .load_config()
        .expect_success();
    let toks = Lexer::new(
        metadata.region_id,
        &metadata.src_bytes,
        metadata.script_start,
        &mut ChrnConfig::default(),
    )
    .tokenize(&mut interner)
    .toks;

    assert_eq!(2, toks.len());
    match toks[0].tok {
        Token::Float(id, Notation::Decimal) => {
            assert_eq!("3.14", interner.search(id));
        }
        _ => panic!("Expected Float with Decimal, found {:?}", toks[0].tok),
    }
    assert_eq!(toks[0].span.start, 0);
    assert_eq!(toks[0].span.end, 4);
    assert_eq!(toks[1].tok, Token::EOF);
    assert_eq!(toks[1].span.start, 3);

    // Positive Scientific Notation
    let text = "1e+23";
    let metadata = ConfigLoader::new(region_id, text.as_bytes(), path_id, &ChrnConfig::default())
        .load_config()
        .expect_success();
    let toks = Lexer::new(
        metadata.region_id,
        &metadata.src_bytes,
        metadata.script_start,
        &mut ChrnConfig::default(),
    )
    .tokenize(&mut interner)
    .toks;

    assert_eq!(2, toks.len());
    match toks[0].tok {
        Token::Float(id, Notation::Decimal) => {
            assert_eq!("1e+23", interner.search(id));
        }
        _ => panic!("Expected Float with Decimal, found {:?}", toks[0].tok),
    }
    assert_eq!(toks[0].span.start, 0);
    assert_eq!(toks[0].span.end, 5);
    assert_eq!(toks[1].tok, Token::EOF);
    assert_eq!(toks[1].span.start, 4);

    // Negative Scientific Notation
    let text = "1e-23";
    let metadata = ConfigLoader::new(region_id, text.as_bytes(), path_id, &ChrnConfig::default())
        .load_config()
        .expect_success();
    let toks = Lexer::new(
        metadata.region_id,
        &metadata.src_bytes,
        metadata.script_start,
        &mut ChrnConfig::default(),
    )
    .tokenize(&mut interner)
    .toks;

    assert_eq!(2, toks.len());
    match toks[0].tok {
        Token::Float(id, Notation::Decimal) => {
            assert_eq!("1e-23", interner.search(id));
        }
        _ => panic!("Expected Float with Decimal, found {:?}", toks[0].tok),
    }
    assert_eq!(toks[0].span.start, 0);
    assert_eq!(toks[0].span.end, 5);
    assert_eq!(toks[1].tok, Token::EOF);
    assert_eq!(toks[1].span.start, 4);

    // Underscored Numbers
    let text = "1_000_000";
    let metadata = ConfigLoader::new(region_id, text.as_bytes(), path_id, &ChrnConfig::default())
        .load_config()
        .expect_success();
    let toks = Lexer::new(
        metadata.region_id,
        &metadata.src_bytes,
        metadata.script_start,
        &mut ChrnConfig::default(),
    )
    .tokenize(&mut interner)
    .toks;

    assert_eq!(2, toks.len());
    match toks[0].tok {
        Token::Integer(id, Notation::Decimal) => {
            assert_eq!("1000000", interner.search(id));
        }
        _ => panic!("Expected Integer with Decimal, found {:?}", toks[0].tok),
    }
    assert_eq!(toks[0].span.start, 0);
    assert_eq!(toks[0].span.end, 9);
    assert_eq!(toks[1].tok, Token::EOF);
    assert_eq!(toks[1].span.start, 8);

    // Underscored Hex
    let text = "0x_ff_ff";
    let metadata = ConfigLoader::new(region_id, text.as_bytes(), path_id, &ChrnConfig::default())
        .load_config()
        .expect_success();
    let toks = Lexer::new(
        metadata.region_id,
        &metadata.src_bytes,
        metadata.script_start,
        &mut ChrnConfig::default(),
    )
    .tokenize(&mut interner)
    .toks;

    assert_eq!(2, toks.len());
    match toks[0].tok {
        Token::Integer(id, Notation::Hex) => {
            assert_eq!("65535", interner.search(id));
        }
        _ => panic!("Expected Integer with Hex, found {:?}", toks[0].tok),
    }
    assert_eq!(toks[0].span.start, 0);
    assert_eq!(toks[0].span.end, 8);
    assert_eq!(toks[1].tok, Token::EOF);
    assert_eq!(toks[1].span.start, 7);
}

#[test]
fn read_ident_includes_trailing_underscore() {
    let src: &[u8] = b"foo_";
    let mut interner = Intern::init();
    let mut cfg = ChrnConfig::default();
    let mut lex = Lexer::new(SourceRegionId::new(0), src, 0, &mut cfg);
    let toks = lex.tokenize(&mut interner).toks;

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
    let mut cfg = ChrnConfig::default();
    let mut lex = Lexer::new(SourceRegionId::new(0), src, 0, &mut cfg);
    let toks = lex.tokenize(&mut interner).toks;

    let id = toks
        .iter()
        .find_map(|st| match st.tok {
            Token::Id(id) => Some((id, st.span)),
            _ => None,
        })
        .expect("expected an Id token for \"_\"");

    assert_eq!(interner.search(id.0), "_");
    assert_eq!(id.1.start, 0);
    assert_eq!(id.1.end, 1);
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
    let mut cfg = ChrnConfig::default();
    let mut lex = Lexer::new(SourceRegionId::new(0), src, 0, &mut cfg);
    let toks = lex.tokenize(&mut interner).toks;

    let names: Vec<(String, u32, u32)> = toks
        .iter()
        .filter_map(|st| match st.tok {
            Token::Id(id) => Some((interner.search(id).to_string(), st.span.start, st.span.end)),
            _ => None,
        })
        .collect();

    assert_eq!(
        names,
        vec![
            ("foo_bar".to_string(), 0, 7),
            ("_qux".to_string(), 8, 12),
            ("a_b_c_".to_string(), 13, 19),
        ]
    );
}
