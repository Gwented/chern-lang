pub mod analyser;
pub mod backend;
pub mod definition;
pub mod document;
pub mod hover;
pub mod state;
pub mod text;

#[cfg(test)]
pub mod tests {
    use chrn_utils::intern::Intern;
    use chrn_utils::keywords::Keyword;

    use script_lib::lexer::Lexer;
    use script_lib::token::{Notation, Token};

    #[test]
    fn lex_bind_and_string() {
        let text = r#"bind "./some/path""#;

        let mut interner = Intern::init();
        let mut lexer = Lexer::new(text.as_bytes(), 0);
        let toks = lexer.tokenize(&mut interner);

        assert!(toks.len() >= 2, "expected at least two tokens");

        match toks[0].tok {
            Token::Keyword(k) => assert_eq!(k, Keyword::Bind),
            other => panic!("expected Keyword(Bind), got {:?}", other),
        }

        match toks[1].tok {
            Token::Str(id) => {
                let s = interner.search(id as usize);
                assert_eq!(s, "./some/path");
            }
            other => panic!("expected Str token, got {:?}", other),
        }
    }

    #[test]
    fn lex_numeric_notations() {
        // Hex
        let text = "0xff";
        let mut interner = Intern::init();
        let mut lexer = Lexer::new(text.as_bytes(), 0);
        let toks = lexer.tokenize(&mut interner);

        assert!(toks.len() >= 1);
        match toks[0].tok {
            Token::Integer(id, Notation::Hex) => {
                let s = interner.search(id as usize);
                assert_eq!(s, "255");
            }
            other => panic!("expected Integer(hex), got {:?}", other),
        }

        // Decimal
        let text = "42";
        let mut interner = Intern::init();
        let mut lexer = Lexer::new(text.as_bytes(), 0);
        let toks = lexer.tokenize(&mut interner);

        assert!(toks.len() >= 1);
        match toks[0].tok {
            Token::Integer(id, Notation::Decimal) => {
                let s = interner.search(id as usize);
                assert_eq!(s, "42");
            }
            other => panic!("expected Integer(decimal), got {:?}", other),
        }
    }
}
