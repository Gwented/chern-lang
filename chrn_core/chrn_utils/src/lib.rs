pub mod chrn_settings;
pub mod config_loader;
pub mod core_error;
pub mod fmter;
pub mod help_model;
pub mod id_types;
pub mod inner_args;
pub mod intern;
pub mod keywords;
pub mod lang_config;
pub mod source_map;
pub mod types;
pub mod values;

#[cfg(test)]
pub mod tests {
    use crate::intern::{self, Intern};

    #[test]
    fn keyword_intern_alignment() {
        let interner = Intern::init();

        assert_eq!(
            "self",
            interner.search_idx(intern::INTERNED_SELF as usize),
            "INTERNED_SELF (0) should be 'self'"
        );
        assert_eq!(
            "struct",
            interner.search_idx(intern::INTERNED_STRUCT as usize),
            "INTERNED_STRUCT (1) should be 'struct'"
        );
        assert_eq!(
            "import",
            interner.search_idx(intern::INTERNED_IMPORT as usize),
            "INTERNED_IMPORT (2) should be 'import'"
        );
        assert_eq!(
            "export",
            interner.search_idx(intern::INTERNED_EXPORT as usize),
            "INTERNED_EXPORT (3) should be 'export'"
        );
        assert_eq!(
            "bind",
            interner.search_idx(intern::INTERNED_BIND as usize),
            "INTERNED_BIND (4) should be 'bind'"
        );
        assert_eq!(
            "alias",
            interner.search_idx(intern::INTERNED_ALIAS as usize),
            "INTERNED_ALIAS (5) should be 'alias'"
        );
        assert_eq!(
            "let",
            interner.search_idx(intern::INTERNED_LET as usize),
            "INTERNED_LET (6) should be 'let'"
        );
        assert_eq!(
            "change",
            interner.search_idx(intern::INTERNED_CHANGE as usize),
            "INTERNED_CHANGE (7) should be 'change'"
        );
        assert_eq!(
            "as",
            interner.search_idx(intern::INTERNED_AS as usize),
            "INTERNED_AS (8) should be 'as'"
        );
        assert_eq!(
            "var",
            interner.search_idx(intern::INTERNED_VAR as usize),
            "INTERNED_VAR (9) should be 'var'"
        );
        assert_eq!(
            "nest",
            interner.search_idx(intern::INTERNED_NEST as usize),
            "INTERNED_NEST (10) should be 'nest'"
        );
        assert_eq!(
            "complex",
            interner.search_idx(intern::INTERNED_COMPLEX as usize),
            "INTERNED_COMPLEX (11) should be 'complex'"
        );
        assert_eq!(
            "override",
            interner.search_idx(intern::INTERNED_OVERRIDE as usize),
            "INTERNED_OVERRIDE (12) should be 'override'"
        );
        assert_eq!(
            "true",
            interner.search_idx(intern::INTERNED_TRUE as usize),
            "INTERNED_TRUE (13) should be 'true'"
        );
        assert_eq!(
            "false",
            interner.search_idx(intern::INTERNED_FALSE as usize),
            "INTERNED_FALSE (14) should be 'false'"
        );
        assert_eq!(
            "IsEmpty",
            interner.search_idx(intern::INTERNED_IS_EMPTY as usize),
            "INTERNED_IS_EMPTY (15) should be 'IsEmpty'"
        );
        assert_eq!(
            "IsWhitespace",
            interner.search_idx(intern::INTERNED_IS_WHITESPACE as usize),
            "INTERNED_IS_WHITESPACE (16) should be 'IsWhitespace'"
        );
        assert_eq!(
            "Range",
            interner.search_idx(intern::INTERNED_RANGE as usize),
            "INTERNED_RANGE (17) should be 'Range'"
        );
        assert_eq!(
            "StartsW",
            interner.search_idx(intern::INTERNED_STARTSW as usize),
            "INTERNED_STARTSW (18) should be 'StartsW'"
        );
        assert_eq!(
            "EndsW",
            interner.search_idx(intern::INTERNED_ENDSW as usize),
            "INTERNED_ENDSW (19) should be 'EndsW'"
        );
        assert_eq!(
            "Contains",
            interner.search_idx(intern::INTERNED_CONTAINS as usize),
            "INTERNED_CONTAINS (20) should be 'Contains'"
        );
        assert_eq!(
            "Equals",
            interner.search_idx(intern::INTERNED_EQUALS as usize),
            "INTERNED_EQUALS (21) should be 'Equals'"
        );
    }

    #[test]
    fn builtin_type_intern_alignment() {
        let interner = Intern::init();

        assert_eq!(
            "i8",
            interner.search_idx(intern::INTERNED_I8 as usize),
            "INTERNED_I8 (22) should be 'i8'"
        );
        assert_eq!(
            "u8",
            interner.search_idx(intern::INTERNED_U8 as usize),
            "INTERNED_U8 (23) should be 'u8'"
        );
        assert_eq!(
            "i16",
            interner.search_idx(intern::INTERNED_I16 as usize),
            "INTERNED_I16 (24) should be 'i16'"
        );
        assert_eq!(
            "u16",
            interner.search_idx(intern::INTERNED_U16 as usize),
            "INTERNED_U16 (25) should be 'u16'"
        );
        assert_eq!(
            "i32",
            interner.search_idx(intern::INTERNED_I32 as usize),
            "INTERNED_I32 (26) should be 'i32'"
        );
        assert_eq!(
            "u32",
            interner.search_idx(intern::INTERNED_U32 as usize),
            "INTERNED_U32 (27) should be 'u32'"
        );
        assert_eq!(
            "f32",
            interner.search_idx(intern::INTERNED_F32 as usize),
            "INTERNED_F32 (28) should be 'f32'"
        );
        assert_eq!(
            "i64",
            interner.search_idx(intern::INTERNED_I64 as usize),
            "INTERNED_I64 (29) should be 'i64'"
        );
        assert_eq!(
            "u64",
            interner.search_idx(intern::INTERNED_U64 as usize),
            "INTERNED_U64 (30) should be 'u64'"
        );
        assert_eq!(
            "f64",
            interner.search_idx(intern::INTERNED_F64 as usize),
            "INTERNED_F64 (31) should be 'f64'"
        );
        assert_eq!(
            "i128",
            interner.search_idx(intern::INTERNED_I128 as usize),
            "INTERNED_I128 (32) should be 'i128'"
        );
        assert_eq!(
            "u128",
            interner.search_idx(intern::INTERNED_U128 as usize),
            "INTERNED_U128 (33) should be 'u128'"
        );
        assert_eq!(
            "f128",
            interner.search_idx(intern::INTERNED_F128 as usize),
            "INTERNED_F128 (34) should be 'f128'"
        );
        assert_eq!(
            "sized",
            interner.search_idx(intern::INTERNED_SIZED as usize),
            "INTERNED_SIZED (35) should be 'sized'"
        );
        assert_eq!(
            "unsized",
            interner.search_idx(intern::INTERNED_UNSIZED as usize),
            "INTERNED_UNSIZED (36) should be 'unsized'"
        );
        assert_eq!(
            "bool",
            interner.search_idx(intern::INTERNED_BOOL as usize),
            "INTERNED_BOOL (37) should be 'bool'"
        );
        assert_eq!(
            "nil",
            interner.search_idx(intern::INTERNED_NIL as usize),
            "INTERNED_NIL (38) should be 'nil'"
        );
        assert_eq!(
            "char",
            interner.search_idx(intern::INTERNED_CHAR as usize),
            "INTERNED_CHAR (39) should be 'char'"
        );
        assert_eq!(
            "str",
            interner.search_idx(intern::INTERNED_STR as usize),
            "INTERNED_STR (40) should be 'str'"
        );
        assert_eq!(
            "BigInt",
            interner.search_idx(intern::INTERNED_BIGINT as usize),
            "INTERNED_BIGINT (41) should be 'BigInt'"
        );
        assert_eq!(
            "BigFloat",
            interner.search_idx(intern::INTERNED_BIGFLOAT as usize),
            "INTERNED_BIGFLOAT (42) should be 'BigFloat'"
        );
        assert_eq!(
            "List",
            interner.search_idx(intern::INTERNED_LIST as usize),
            "INTERNED_LIST (43) should be 'List'"
        );
        assert_eq!(
            "Set",
            interner.search_idx(intern::INTERNED_SET as usize),
            "INTERNED_SET (44) should be 'Set'"
        );
        assert_eq!(
            "Map",
            interner.search_idx(intern::INTERNED_MAP as usize),
            "INTERNED_MAP (45) should be 'Map'"
        );
        assert_eq!(
            "Tuple",
            interner.search_idx(intern::INTERNED_TUPLE as usize),
            "INTERNED_TUPLE (46) should be 'Tuple'"
        );
    }

    #[test]
    fn builtin_type_kind_alignment() {
        let interner = Intern::init();

        assert_eq!(interner.search_idx(intern::INTERNED_I8 as usize), "i8");
        assert_eq!(interner.search_idx(intern::INTERNED_U8 as usize), "u8");
        assert_eq!(interner.search_idx(intern::INTERNED_I16 as usize), "i16");
        assert_eq!(interner.search_idx(intern::INTERNED_U16 as usize), "u16");
        assert_eq!(interner.search_idx(intern::INTERNED_I32 as usize), "i32");
        assert_eq!(interner.search_idx(intern::INTERNED_U32 as usize), "u32");
        assert_eq!(interner.search_idx(intern::INTERNED_F32 as usize), "f32");
        assert_eq!(interner.search_idx(intern::INTERNED_I64 as usize), "i64");
        assert_eq!(interner.search_idx(intern::INTERNED_U64 as usize), "u64");
        assert_eq!(interner.search_idx(intern::INTERNED_F64 as usize), "f64");
        assert_eq!(interner.search_idx(intern::INTERNED_I128 as usize), "i128");
        assert_eq!(interner.search_idx(intern::INTERNED_U128 as usize), "u128");
        assert_eq!(interner.search_idx(intern::INTERNED_F128 as usize), "f128");
        assert_eq!(
            interner.search_idx(intern::INTERNED_SIZED as usize),
            "sized"
        );
        assert_eq!(
            interner.search_idx(intern::INTERNED_UNSIZED as usize),
            "unsized"
        );
        assert_eq!(interner.search_idx(intern::INTERNED_BOOL as usize), "bool");
        assert_eq!(interner.search_idx(intern::INTERNED_NIL as usize), "nil");
        assert_eq!(interner.search_idx(intern::INTERNED_CHAR as usize), "char");
        assert_eq!(interner.search_idx(intern::INTERNED_STR as usize), "str");
        assert_eq!(
            interner.search_idx(intern::INTERNED_BIGINT as usize),
            "BigInt"
        );
        assert_eq!(
            interner.search_idx(intern::INTERNED_BIGFLOAT as usize),
            "BigFloat"
        );
        assert_eq!(interner.search_idx(intern::INTERNED_LIST as usize), "List");
        assert_eq!(interner.search_idx(intern::INTERNED_SET as usize), "Set");
        assert_eq!(interner.search_idx(intern::INTERNED_MAP as usize), "Map");
    }

    #[test]
    fn interned_preload_size_matches() {
        assert_eq!(
            intern::INTERNED_CORE + 1,
            51,
            "INTERNER_PRELOAD_SIZE should match number of preloaded interned strings"
        );
    }
}
