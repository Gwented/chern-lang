use std::ops::RangeInclusive;
//WARN: WAY TOO MANY MICRO-DEPENDENCIES

// Before adding a keyword:
// Ensure array string is aligned with the Keyword enum
// Ensure ranges are adjusted
// Ensure tests are aligned

pub static KEYWORDS_ARRAY: [&str; 41] = [
    // primitives
    "i8", // 0
    "u8",
    "i16",
    "u16",
    "f16", // 4
    "i32",
    "u32", // 6
    "f32",
    "i64", // 8
    "u64",
    "f64", // 10
    "i128",
    "u128", // 12
    "f128",
    "sized", // 14
    "unsized",
    "char", // 16
    "str",
    "bool", // 18
    "nil",
    "BigInt", // 20
    "BigFloat",
    "List",
    "Map",
    "Set", // 24
    // structures
    "struct",
    "enum", // 26
    // Directives?
    "import",
    "export",
    // Statements
    "bind",
    "alias", // 30
    // Section names
    "var",
    "nest", // 32
    "complex",
    "override", // 34
    // Predicate keywords
    "IsEmpty",
    "IsWhitespace", // 36
    // Predicates (Function)
    "Range",
    "StartsW", // 38
    "EndsW",
    "Contains", // 40
];

// Keep a compact enum for code that prefers typed keyword identifiers.
// I think I don't know I am new to thinking does anyone have beginner thoughts?
#[derive(Clone, Copy, Debug)]
#[repr(u32)]
pub enum Keyword {
    I8 = 0,
    U8 = 1,
    I16 = 2,
    U16 = 3,
    F16 = 4,
    I32 = 5,
    U32 = 6,
    F32 = 7,
    I64 = 8,
    U64 = 9,
    F64 = 10,
    I128 = 11,
    U128 = 12,
    F128 = 13,
    Sized = 14,
    Unsized = 15,
    Char = 16,
    Str = 17,
    Bool = 18,
    Nil = 19,
    BigInt = 20,
    BigFloat = 21,
    List = 22,
    Map = 23,
    Set = 24,
    Struct = 25,
    Enum = 26,
    Import = 27,
    Export = 28,
    Bind = 29,
    Alias = 30,
    Var = 31,
    Nest = 32,
    Complex = 33,
    Override = 34,
    IsEmpty = 35,
    IsWhitespace = 36,
    Range = 37,
    StartsW = 38,
    EndsW = 39,
    Contains = 40,
}

impl Keyword {
    pub fn try_as_kw(id: u32) -> Option<Keyword> {
        match id {
            // Using literal because scared of if
            0 => Some(Keyword::I8),
            1 => Some(Keyword::U8),
            2 => Some(Keyword::I16),
            3 => Some(Keyword::U16),
            4 => Some(Keyword::F16),
            5 => Some(Keyword::I32),
            6 => Some(Keyword::U32),
            7 => Some(Keyword::F32),
            8 => Some(Keyword::I64),
            9 => Some(Keyword::U64),
            10 => Some(Keyword::F64),
            11 => Some(Keyword::I128),
            12 => Some(Keyword::U128),
            13 => Some(Keyword::F128),
            14 => Some(Keyword::Sized),
            15 => Some(Keyword::Unsized),
            16 => Some(Keyword::Char),
            17 => Some(Keyword::Str),
            18 => Some(Keyword::Bool),
            19 => Some(Keyword::Nil),
            20 => Some(Keyword::BigInt),
            21 => Some(Keyword::BigFloat),
            22 => Some(Keyword::List),
            23 => Some(Keyword::Map),
            24 => Some(Keyword::Set),
            25 => Some(Keyword::Struct),
            26 => Some(Keyword::Enum),
            27 => Some(Keyword::Import),
            28 => Some(Keyword::Export),
            29 => Some(Keyword::Bind),
            30 => Some(Keyword::Alias),
            31 => Some(Keyword::Var),
            32 => Some(Keyword::Nest),
            33 => Some(Keyword::Complex),
            34 => Some(Keyword::Override),
            35 => Some(Keyword::IsEmpty),
            36 => Some(Keyword::IsWhitespace),
            37 => Some(Keyword::Range),
            38 => Some(Keyword::StartsW),
            39 => Some(Keyword::EndsW),
            40 => Some(Keyword::Contains),
            _ => None,
        }
    }

    pub fn try_as_builtin(id: u32) -> Option<Keyword> {
        if let Some(kw) = Self::try_as_kw(id) {
            match kw {
                Keyword::I8
                | Keyword::U8
                | Keyword::I16
                | Keyword::U16
                | Keyword::F16
                | Keyword::I32
                | Keyword::U32
                | Keyword::F32
                | Keyword::I64
                | Keyword::U64
                | Keyword::F64
                | Keyword::I128
                | Keyword::U128
                | Keyword::F128
                | Keyword::Sized
                | Keyword::Unsized
                | Keyword::Char
                | Keyword::Str
                | Keyword::Bool
                | Keyword::Nil
                | Keyword::BigInt
                | Keyword::BigFloat => return Some(kw),
                _ => return None,
            }
        }

        None
    }

    pub fn try_as_data_struct(id: u32) -> Option<Keyword> {
        if let Some(kw) = Self::try_as_kw(id) {
            match kw {
                Keyword::List | Keyword::Map => todo!(),
                Keyword::Set => return Some(kw),
                _ => return None,
            }
        }

        None
    }

    // pub fn try_as_cond(id: u32) -> Option<Keyword> {
    //     if let Some(kw) = Self::try_as_kw(id) {
    //         match kw {
    //             Keyword::IsEmpty
    //             | Keyword::IsWhitespace
    //             | Keyword::Range
    //             | Keyword::StartsW
    //             | Keyword::EndsW
    //             | Keyword::Contains => return Some(kw),
    //             _ => return None,
    //         }
    //     }
    //
    //     None
    // }
}

//WARN: Not sure about the amount of casting everywhere
const TYPE_START: u32 = 0;
const TYPE_END: u32 = 24;

const SECT_START: u32 = 31;
const SECT_END: u32 = 34;

const STMT_START: u32 = 29;
const STMT_END: u32 = 30;

const PREDICATE_START: u32 = 35;
const PREDICATE_END: u32 = 40;

//WARN: The amount of casting here is painful. SEVERELY painful.
pub fn is_type(id: u32) -> bool {
    id <= TYPE_END
}

pub fn is_sect(id: u32) -> bool {
    (SECT_START..=SECT_END).contains(&id)
}

pub fn type_range() -> RangeInclusive<usize> {
    (TYPE_START as usize)..=(TYPE_END as usize)
}

pub fn stmt_range() -> RangeInclusive<usize> {
    (STMT_START as usize)..=(STMT_END as usize)
}

pub fn sect_range() -> RangeInclusive<usize> {
    (SECT_START as usize)..=(SECT_END as usize)
}

pub fn predicate_range() -> RangeInclusive<usize> {
    (PREDICATE_START as usize)..=(PREDICATE_END as usize)
}
