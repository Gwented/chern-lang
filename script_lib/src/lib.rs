// Should this be pub(crate)?
mod algo;
pub mod lexer;
pub mod linter;
pub mod parser;
pub mod semantic;
pub mod types;

#[cfg(test)]
mod tests {
    use std::path::Path;

    use common::{intern::Intern, storage::FileLoader};

    use crate::{
        lexer::Lexer,
        types::token::{Notation, Token},
    };

    #[test]
    fn lex_tok_test() {
        let text = r#"bind "./some/path""#;
        dbg!(&text);

        let metadata = FileLoader::new(Path::new(""), text.as_bytes())
            .load_config()
            .unwrap();

        let mut interner = Intern::init();

        let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

        for tok in &toks {
            match tok.token {
                Token::Id(id) | Token::Str(id) => {
                    dbg!(interner.search(id as usize));
                }
                _ => (),
            }
        }

        assert_eq!(
            0, metadata.serial_start,
            "start_offset without `@def` failed"
        );
        assert_eq!(3, toks.len(), "Token length exceeded 3 in lex_tok_test");
    }

    #[test]
    fn lex_tok_test_rev() {
        // Properly closed @def and @end
        let correct = r#"@defbind "./some/path"@end"#;

        let opt = FileLoader::new(Path::new(""), correct.as_bytes()).load_config();

        assert_eq!(true, opt.is_ok());

        // Improper @def without an @end
        let wrong = r#"@defbind "./some/path""#;

        let opt = FileLoader::new(Path::new(""), wrong.as_bytes()).load_config();

        assert_eq!(true, opt.is_err());
    }

    //utf8 broke

    #[test]
    fn char_literal_test() {
        // Valid single character
        let text = "'a'";
        let metadata = FileLoader::new(Path::new(""), text.as_bytes())
            .load_config()
            .unwrap();
        let mut interner = Intern::init();
        let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

        assert_eq!(2, toks.len());
        assert!(
            matches!(toks[0].token, Token::Char(_),),
            "Expected char token, got {:?}",
            toks[0].token
        );

        // Valid escaped character
        let text = "'\\n'";
        let metadata = FileLoader::new(Path::new(""), text.as_bytes())
            .load_config()
            .unwrap();
        let mut interner = Intern::init();
        let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

        assert_eq!(2, toks.len());
        assert!(
            matches!(toks[0].token, Token::Char(_),),
            "Expected char token, got {:?}",
            toks[0].token
        );

        // Valid hex escape
        let text = "'\\x2F'";
        let metadata = FileLoader::new(Path::new(""), text.as_bytes())
            .load_config()
            .unwrap();
        let mut interner = Intern::init();
        let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

        assert_eq!(2, toks.len());
        assert!(
            matches!(toks[0].token, Token::Char(_),),
            "Expected char token, got {:?}",
            toks[0].token
        );

        // Invalid character
        let text = "'aa'";
        let metadata = FileLoader::new(Path::new(""), text.as_bytes())
            .load_config()
            .unwrap();
        let mut interner = Intern::init();
        let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

        assert_eq!(2, toks.len());
        assert!(
            matches!(toks[0].token, Token::Illegal(_),),
            "Expected Illegal token, got {:?}",
            toks[0].token
        );

        // Invalid hex escape
        let text = "'\\x2'";
        let metadata = FileLoader::new(Path::new(""), text.as_bytes())
            .load_config()
            .unwrap();
        let mut interner = Intern::init();
        let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

        assert_eq!(2, toks.len());
        assert!(
            matches!(toks[0].token, Token::Illegal(_),),
            "Expected Illegal token, got {:?}",
            toks[0].token
        );

        // I can't actually read hex
        // Invalid hex digits
        let text = "'\\x255'";
        let metadata = FileLoader::new(Path::new(""), text.as_bytes())
            .load_config()
            .unwrap();
        let mut interner = Intern::init();
        let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

        assert_eq!(2, toks.len());
        assert!(
            matches!(toks[0].token, Token::Illegal(_),),
            "Expected Illegal token, got {:?}",
            toks[0].token
        );

        // Unknown escape
        let text = "'\\q'";
        let metadata = FileLoader::new(Path::new(""), text.as_bytes())
            .load_config()
            .unwrap();
        let mut interner = Intern::init();
        let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

        assert_eq!(2, toks.len());
        assert!(
            matches!(toks[0].token, Token::Illegal(_),),
            "Expected Illegal token, got {:?}",
            toks[0].token
        );

        // Out of range escape
        let text = "'\\x1Y'";
        let metadata = FileLoader::new(Path::new(""), text.as_bytes())
            .load_config()
            .unwrap();
        let mut interner = Intern::init();
        let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

        assert_eq!(2, toks.len());
        assert!(
            matches!(toks[0].token, Token::Illegal(_),),
            "Expected Illegal token, got {:?}",
            toks[0].token
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

        let correct = FileLoader::new(Path::new(""), correct).load_config();
        let wrong = FileLoader::new(Path::new(""), wrong).load_config();

        assert_eq!(true, correct.is_ok());
        assert_eq!(true, wrong.is_err());
    }

    // beautiful name
    #[test]
    fn start_and_serial_offset_test() {
        let text = format!("adwh@def var-> int: i32 @endhi");

        let metadata = FileLoader::new(Path::new(""), text.as_bytes())
            .load_config()
            .unwrap();

        assert_eq!(&text[4..], &text[metadata.lex_start..]);
        assert_eq!("hi", &text[metadata.serial_start..]);
        assert_eq!(28, metadata.serial_start);
    }

    #[test]
    fn lex_notation_test() {
        // Hex Test (Hex Text (Hex Test))
        let text = "0xff";
        let metadata = FileLoader::new(Path::new(""), text.as_bytes())
            .load_config()
            .unwrap();
        let mut interner = Intern::init();
        let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

        assert_eq!(2, toks.len());
        match toks[0].token {
            Token::Integer(id, Notation::Hex) => {
                assert_eq!("255", interner.search(id as usize));
            }
            _ => panic!("Expected Integer with Hex, found {:?}", toks[0].token),
        }

        // Binary
        let text = "0b1010";
        let metadata = FileLoader::new(Path::new(""), text.as_bytes())
            .load_config()
            .unwrap();
        let mut interner = Intern::init();
        let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

        assert_eq!(2, toks.len());
        match toks[0].token {
            Token::Integer(id, Notation::Bin) => {
                assert_eq!("10", interner.search(id as usize));
            }
            _ => panic!("Expected Integer with Binary, found {:?}", toks[0].token),
        }

        // Octal
        let text = "0o77";
        let metadata = FileLoader::new(Path::new(""), text.as_bytes())
            .load_config()
            .unwrap();
        let mut interner = Intern::init();
        let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

        assert_eq!(2, toks.len());
        match toks[0].token {
            Token::Integer(id, Notation::Octal) => {
                assert_eq!("63", interner.search(id as usize));
            }
            _ => panic!("Expected Integer with Octal, found {:?}", toks[0].token),
        }

        // Decimal
        let text = "42";
        let metadata = FileLoader::new(Path::new(""), text.as_bytes())
            .load_config()
            .unwrap();
        let mut interner = Intern::init();
        let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

        assert_eq!(2, toks.len());
        match toks[0].token {
            Token::Integer(id, Notation::Decimal) => {
                assert_eq!("42", interner.search(id as usize));
            }
            _ => panic!("Expected Integer with Decimal, found {:?}", toks[0].token),
        }

        // Float with decimal
        let text = "3.14";
        let metadata = FileLoader::new(Path::new(""), text.as_bytes())
            .load_config()
            .unwrap();
        let mut interner = Intern::init();
        let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

        assert_eq!(2, toks.len());
        match toks[0].token {
            Token::Float(id, Notation::Decimal) => {
                assert_eq!("3.14", interner.search(id as usize));
            }
            _ => panic!("Expected Float with Decimal, found {:?}", toks[0].token),
        }

        // Positive Scientific Notation
        let text = "1e+23";
        let metadata = FileLoader::new(Path::new(""), text.as_bytes())
            .load_config()
            .unwrap();
        let mut interner = Intern::init();
        let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

        assert_eq!(2, toks.len());
        match toks[0].token {
            Token::Float(id, Notation::Decimal) => {
                assert_eq!("1e+23", interner.search(id as usize));
            }
            _ => panic!("Expected Float with Decimal, found {:?}", toks[0].token),
        }

        // Negative Scientific Notation
        let text = "1e-23";
        let metadata = FileLoader::new(Path::new(""), text.as_bytes())
            .load_config()
            .unwrap();
        let mut interner = Intern::init();
        let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

        assert_eq!(2, toks.len());
        match toks[0].token {
            Token::Float(id, Notation::Decimal) => {
                assert_eq!("1e-23", interner.search(id as usize));
            }
            _ => panic!("Expected Float with Decimal, found {:?}", toks[0].token),
        }

        // Underscored Numbers
        let text = "1_000_000";
        let metadata = FileLoader::new(Path::new(""), text.as_bytes())
            .load_config()
            .unwrap();
        let mut interner = Intern::init();
        let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

        assert_eq!(2, toks.len());
        match toks[0].token {
            Token::Integer(id, Notation::Decimal) => {
                assert_eq!("1000000", interner.search(id as usize));
            }
            _ => panic!("Expected Integer with Decimal, found {:?}", toks[0].token),
        }

        // Underscored Hex
        let text = "0x_ff_ff";
        let metadata = FileLoader::new(Path::new(""), text.as_bytes())
            .load_config()
            .unwrap();
        let mut interner = Intern::init();
        let toks = Lexer::new(&metadata.src_bytes, metadata.lex_start).tokenize(&mut interner);

        assert_eq!(2, toks.len());
        match toks[0].token {
            Token::Integer(id, Notation::Hex) => {
                assert_eq!("65535", interner.search(id as usize));
            }
            _ => panic!("Expected Integer with Hex, found {:?}", toks[0].token),
        }
    }
}
