// Misleading...
pub mod builtins;
pub mod color;
pub mod config_loader;
pub mod core_error;
pub mod fmter;
pub mod help_model;
pub mod intern;
pub mod keywords;
pub mod lang_config;
pub mod metadata;
pub mod reporter;
pub mod symbols;

// WAIT HOW DO I TEST THIS
#[cfg(test)]
pub mod tests {
    use crate::{
        intern::Intern,
        keywords::{self, Keyword},
    };

    #[test]
    pub fn primitives_test() {
        let interner = Intern::init();

        // Should this stay?
        // Types
        assert_eq!("i8", interner.search(Keyword::I8 as usize));
        assert_eq!("u8", interner.search(Keyword::U8 as usize));
        assert_eq!("i16", interner.search(Keyword::I16 as usize));
        assert_eq!("u16", interner.search(Keyword::U16 as usize));
        assert_eq!("f16", interner.search(Keyword::F16 as usize));
        assert_eq!("i32", interner.search(Keyword::I32 as usize));
        assert_eq!("u32", interner.search(Keyword::U32 as usize));
        assert_eq!("f32", interner.search(Keyword::F32 as usize));
        assert_eq!("i64", interner.search(Keyword::I64 as usize));
        assert_eq!("u64", interner.search(Keyword::U64 as usize));
        assert_eq!("f64", interner.search(Keyword::F64 as usize));
        assert_eq!("i128", interner.search(Keyword::I128 as usize));
        assert_eq!("u128", interner.search(Keyword::U128 as usize));
        assert_eq!("f128", interner.search(Keyword::F128 as usize));
        assert_eq!("sized", interner.search(Keyword::Sized as usize));
        // Thank you formatter for making this harder to read
        assert_eq!("unsized", interner.search(Keyword::Unsized as usize));
        assert_eq!("char", interner.search(Keyword::Char as usize));
        assert_eq!("str", interner.search(Keyword::Str as usize));
        assert_eq!("bool", interner.search(Keyword::Bool as usize));
        assert_eq!("nil", interner.search(Keyword::Nil as usize));
        assert_eq!("BigInt", interner.search(Keyword::BigInt as usize));
        assert_eq!("BigFloat", interner.search(Keyword::BigFloat as usize));
        assert_eq!("List", interner.search(Keyword::List as usize));
        assert_eq!("Map", interner.search(Keyword::Map as usize));
        assert_eq!("Set", interner.search(Keyword::Set as usize));
        assert_eq!("Tuple", interner.search(Keyword::Tuple as usize));
        // Structures
        assert_eq!("self", interner.search(Keyword::Self_ as usize));
        assert_eq!("struct", interner.search(Keyword::Struct as usize));
        assert_eq!("enum", interner.search(Keyword::Enum as usize));
        // I have never used JS or TS in any serious manner
        // Statements
        assert_eq!("import", interner.search(Keyword::Import as usize));
        assert_eq!("export", interner.search(Keyword::Export as usize));
        assert_eq!("bind", interner.search(Keyword::Bind as usize));
        assert_eq!("alias", interner.search(Keyword::Alias as usize));
        assert_eq!("const", interner.search(Keyword::Const as usize));
        assert_eq!("change", interner.search(Keyword::Change as usize));
        // Sections
        assert_eq!("var", interner.search(Keyword::Var as usize));
        assert_eq!("nest", interner.search(Keyword::Nest as usize));
        assert_eq!("complex", interner.search(Keyword::Complex as usize));
        assert_eq!("override", interner.search(Keyword::Override as usize));
        // Other keywords
        assert_eq!("as", interner.search(Keyword::As as usize));
        // Keywords & Funcs
        assert_eq!("IsEmpty", interner.search(Keyword::IsEmpty as usize));
        assert_eq!(
            "IsWhitespace",
            interner.search(Keyword::IsWhitespace as usize)
        );
        assert_eq!("Range", interner.search(Keyword::Range as usize));
        assert_eq!("StartsW", interner.search(Keyword::StartsW as usize));
        assert_eq!("EndsW", interner.search(Keyword::EndsW as usize));
        assert_eq!("Contains", interner.search(Keyword::Contains as usize));
        // This COULD use self == thing theoretically but not sure right now
        assert_eq!("Equals", interner.search(Keyword::Equals as usize));

        // Index alignment test
        for (i, kw_str) in keywords::KEYWORDS_ARRAY.iter().enumerate() {
            let kw = Keyword::try_as_kw(i as u32).expect("Issue with Keyword enum numbering");
            let interned_str = interner.search(kw as usize);

            assert_eq!(
                *kw_str, interned_str,
                "Keyword at index {}: expected '{}', found '{}'",
                i, kw_str, interned_str
            );
        }

        assert_eq!(keywords::is_export(Keyword::Export as u32), true);

        assert_eq!(keywords::SECT_START..=keywords::SECT_END, 35..=38);

        // BigFloat is right before data structures, which can't be pre-loaded, so it is the test
        // case that confirms the pre-loaded variables are not anything beyond basic primitives
        assert_eq!(
            keywords::KEYWORDS_ARRAY[(keywords::TYPE_END - 5) as usize],
            "BigFloat"
        );

        // This is to force me to check even if it was done correctly
        assert_eq!(keywords::KEYWORDS_ARRAY.len(), 47);
    }
}
