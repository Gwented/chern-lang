pub mod intern;
pub mod keywords;
pub mod metadata;
pub mod reporter;
pub mod storage;
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
        // Structures
        assert_eq!("struct", interner.search(Keyword::Struct as usize));
        assert_eq!("enum", interner.search(Keyword::Enum as usize));
        // Statements
        assert_eq!("bind", interner.search(Keyword::Bind as usize));
        assert_eq!("alias", interner.search(Keyword::Alias as usize));
        // Sections
        assert_eq!("var", interner.search(Keyword::Var as usize));
        assert_eq!("nest", interner.search(Keyword::Nest as usize));
        assert_eq!("complex", interner.search(Keyword::Complex as usize));
        assert_eq!("override", interner.search(Keyword::Override as usize));
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

        // Uh
        assert_eq!(keywords::KEYWORDS_ARRAY.len(), 39);
    }
}
