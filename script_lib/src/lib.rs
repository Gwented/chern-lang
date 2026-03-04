pub mod lexer;
pub mod linter;
pub mod parser;
pub mod token;

#[cfg(test)]
mod tests {

    use common::{intern::Intern, primitives::PrimitiveKeywords, storage::FileLoader};

    use crate::lexer::Lexer;

    #[test]
    fn lex_tok_test() {
        let text = r#"bind-> "./some/path""#;
        dbg!(&text);

        let (cfg, lex_start, start_offset) =
            FileLoader::new(text.as_bytes()).load_config().unwrap();

        let mut interner = Intern::init();

        let toks = Lexer::new(&cfg, lex_start).tokenize(&mut interner);

        assert_eq!(0, start_offset, "start_offset without `@def` failed");
        assert_eq!(4, toks.len(), "Token length exceeded 4 in lex_tok_test");
    }

    #[test]
    fn lex_tok_test_rev() {
        // let text = r#"bind-> "./some/path""#;
        let text = r#"@defbind-> "./some/path""#;
        dbg!(&text);

        let opt = FileLoader::new(text.as_bytes()).load_config();

        assert_eq!(true, opt.is_err());
    }

    //utf8 broke

    // #[ignore = "Hi"]
    #[test]
    fn primitives_test() {
        let interner = Intern::init();

        assert_eq!("i8", interner.search(PrimitiveKeywords::I8 as usize));
        assert_eq!("u8", interner.search(PrimitiveKeywords::U8 as usize));
        assert_eq!("i16", interner.search(PrimitiveKeywords::I16 as usize));
        assert_eq!("u16", interner.search(PrimitiveKeywords::U16 as usize));
        assert_eq!("f16", interner.search(PrimitiveKeywords::F16 as usize));
        assert_eq!("i32", interner.search(PrimitiveKeywords::I32 as usize));
        assert_eq!("u32", interner.search(PrimitiveKeywords::U32 as usize));
        assert_eq!("f32", interner.search(PrimitiveKeywords::F32 as usize));
        assert_eq!("i64", interner.search(PrimitiveKeywords::I64 as usize));
        assert_eq!("u64", interner.search(PrimitiveKeywords::U64 as usize));
        assert_eq!("f64", interner.search(PrimitiveKeywords::F64 as usize));
        assert_eq!("i128", interner.search(PrimitiveKeywords::I128 as usize));
        assert_eq!("u128", interner.search(PrimitiveKeywords::U128 as usize));
        assert_eq!("f128", interner.search(PrimitiveKeywords::F128 as usize));
        assert_eq!("sized", interner.search(PrimitiveKeywords::Sized as usize));
        // Thank you formatter for making this harder to read
        assert_eq!(
            "unsized",
            interner.search(PrimitiveKeywords::Unsized as usize)
        );
        assert_eq!("char", interner.search(PrimitiveKeywords::Char as usize));
        assert_eq!("str", interner.search(PrimitiveKeywords::Str as usize));
        assert_eq!("bool", interner.search(PrimitiveKeywords::Bool as usize));
        assert_eq!("nil", interner.search(PrimitiveKeywords::Nil as usize));
        assert_eq!(
            "BigInt",
            interner.search(PrimitiveKeywords::BigInt as usize)
        );
        assert_eq!(
            "BigFloat",
            interner.search(PrimitiveKeywords::BigFloat as usize)
        );
        assert_eq!("List", interner.search(PrimitiveKeywords::List as usize));
        assert_eq!("Map", interner.search(PrimitiveKeywords::Map as usize));
        assert_eq!("Set", interner.search(PrimitiveKeywords::Set as usize));
        assert_eq!("bind", interner.search(PrimitiveKeywords::Bind as usize));
        assert_eq!("var", interner.search(PrimitiveKeywords::Var as usize));
        assert_eq!("nest", interner.search(PrimitiveKeywords::Nest as usize));
        assert_eq!(
            "complex_rules",
            interner.search(PrimitiveKeywords::ComplexRules as usize)
        );
        assert_eq!(
            "IsEmpty",
            interner.search(PrimitiveKeywords::IsEmpty as usize)
        );
        assert_eq!(
            "IsWhitespace",
            interner.search(PrimitiveKeywords::IsWhitespace as usize)
        );
        assert_eq!("Range", interner.search(PrimitiveKeywords::Range as usize));
        assert_eq!(
            "StartsW",
            interner.search(PrimitiveKeywords::StartsW as usize)
        );
        assert_eq!("EndsW", interner.search(PrimitiveKeywords::EndsW as usize));
        assert_eq!(
            "Contains",
            interner.search(PrimitiveKeywords::Contains as usize)
        );

        // Uh
        assert_eq!(34, PrimitiveKeywords::Contains as usize);
    }

    #[test]
    fn multi_line_comment_test() {
        let correct = "
            /* /* */ */
        "
        .as_bytes();

        let wrong = "
            /* /* */ 
        "
        .as_bytes();

        let correct = FileLoader::new(correct).load_config();
        let wrong = FileLoader::new(wrong).load_config();

        assert_eq!(true, correct.is_ok());
        assert_eq!(true, wrong.is_err());
    }

    #[test]
    fn start_offset_test() {
        let text = format!("adwh@def var-> int: i32 @endhi");

        let (_, lex_start, serial_offset) = FileLoader::new(text.as_bytes()).load_config().unwrap();

        assert_eq!(&text[4..], &text[lex_start..]);
        assert_eq!("hi", &text[serial_offset..]);
        assert_eq!(28, serial_offset);
    }

    // #[test]
    // fn template_test() {
    //     let text = r#"
    //             person: S|Person,
    //             nest->
    //                 .person {
    //                     name: str
    //                     age: u8
    //                     things: List<i32>
    //                 }
    //             "#;
    //
    //     let (cfg, lex_start, _) = FileLoader::new(text.as_bytes()).load_config().unwrap();
    //
    //     let mut interner = Intern::init();
    //
    //     let toks = Lexer::new(&cfg, lex_start).tokenize(&mut interner);
    //
    //     let sym_table = parser::parse(&cfg, &toks, &mut interner);
    //
    //     dbg!(sym_table);
    //     // assert_eq!("hi", &text[start_offset..]);
    //     // assert_eq!(start_offset, 24, "windows.h");
    // }
}
