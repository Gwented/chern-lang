use std::ops::RangeInclusive;

use crate::fmter::{Formattable, Formatted};

/// Known size in bytes for `@def` and `@end`
pub const DEFINITION_SIZE: usize = 4;
//WARN: WAY TOO MANY MICRO-DEPENDENCIES

// Before adding a keyword:
// Ensure array string is aligned with the Keyword enum
// Ensure ranges are adjusted
// Ensure tests are aligned

//
pub static KEYWORDS_ARRAY: [&str; 45] = [
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
    // Not sure if this should be a different category but can just in structure
    "self",
    // structures
    "struct", // 26
    "enum",
    // Directives?
    "import", // 28
    "export",
    // Statements
    "bind", // 30
    "alias",
    "const", // 32
    "change",
    // Section names
    "var", // 34
    "nest",
    "complex", // 36
    "override",
    // Predicate keywords
    "IsEmpty", // 38
    "IsWhitespace",
    // Predicates (Function)
    "Range", // 40
    "StartsW",
    "EndsW", // 42
    "Contains",
    "Equals", // 44
];
// "Nat" // 37
// "Real" // 38
// "Complex" // 39
// "Prime" // 40

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
    Self_ = 25,
    Struct = 26,
    Enum = 27,
    Import = 28,
    Export = 29,
    Bind = 30,
    Alias = 31,
    Const = 32,
    Change = 33,
    Var = 34,
    Nest = 35,
    Complex = 36,
    Override = 37,
    IsEmpty = 38,
    IsWhitespace = 39,
    Range = 40,
    StartsW = 41,
    EndsW = 42,
    Contains = 43,
    Equals = 44,
}

impl Formattable for Keyword {
    fn to_fmt(&self) -> crate::fmter::Formatted {
        match self {
            Keyword::I8 => Formatted::I8,
            Keyword::U8 => Formatted::U8,
            Keyword::I16 => Formatted::I16,
            Keyword::U16 => Formatted::U16,
            Keyword::F16 => Formatted::F16,
            Keyword::I32 => Formatted::I32,
            Keyword::U32 => Formatted::U32,
            Keyword::F32 => Formatted::F32,
            Keyword::I64 => Formatted::I64,
            Keyword::U64 => Formatted::U64,
            Keyword::F64 => Formatted::F64,
            Keyword::I128 => Formatted::I128,
            Keyword::U128 => Formatted::U128,
            Keyword::F128 => Formatted::F128,
            Keyword::Sized => Formatted::Sized,
            Keyword::Unsized => Formatted::Unsized,
            Keyword::Char => Formatted::Char,
            Keyword::Str => Formatted::Str,
            Keyword::Bool => Formatted::Bool,
            Keyword::Nil => Formatted::Nil,
            Keyword::BigInt => Formatted::BigInt,
            Keyword::BigFloat => Formatted::BigFloat,
            Keyword::List => Formatted::List,
            Keyword::Map => Formatted::Map,
            Keyword::Set => Formatted::Set,
            Keyword::Self_ => Formatted::Self_,
            Keyword::Struct => Formatted::Sized,
            Keyword::Enum => Formatted::Sized,
            Keyword::Import => Formatted::Sized,
            Keyword::Export => Formatted::Export,
            Keyword::Bind => Formatted::Bind,
            Keyword::Alias => Formatted::Alias,
            Keyword::Const => Formatted::Const,
            Keyword::Change => Formatted::Change,
            Keyword::Var => Formatted::Var,
            Keyword::Nest => Formatted::Nest,
            Keyword::Complex => Formatted::Complex,
            Keyword::Override => Formatted::Override,
            Keyword::IsEmpty => Formatted::IsEmpty,
            Keyword::IsWhitespace => Formatted::IsWhitespace,
            Keyword::Range => Formatted::FuncRange,
            Keyword::StartsW => Formatted::FuncStartsW,
            Keyword::EndsW => Formatted::FuncEndsW,
            Keyword::Contains => Formatted::FuncContains,
            Keyword::Equals => Formatted::FuncEquals,
        }
    }
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
            25 => Some(Keyword::Self_),
            26 => Some(Keyword::Struct),
            27 => Some(Keyword::Enum),
            28 => Some(Keyword::Import),
            29 => Some(Keyword::Export),
            30 => Some(Keyword::Bind),
            31 => Some(Keyword::Alias),
            32 => Some(Keyword::Const),
            33 => Some(Keyword::Change),
            34 => Some(Keyword::Var),
            35 => Some(Keyword::Nest),
            36 => Some(Keyword::Complex),
            37 => Some(Keyword::Override),
            38 => Some(Keyword::IsEmpty),
            39 => Some(Keyword::IsWhitespace),
            40 => Some(Keyword::Range),
            41 => Some(Keyword::StartsW),
            42 => Some(Keyword::EndsW),
            43 => Some(Keyword::Contains),
            44 => Some(Keyword::Equals),
            _ => None,
        }
    }

    // pub fn try_as_builtin(id: u32) -> Option<Keyword> {
    //     if let Some(kw) = Self::try_as_kw(id) {
    //         match kw {
    //             Keyword::I8
    //             | Keyword::U8
    //             | Keyword::I16
    //             | Keyword::U16
    //             | Keyword::F16
    //             | Keyword::I32
    //             | Keyword::U32
    //             | Keyword::F32
    //             | Keyword::I64
    //             | Keyword::U64
    //             | Keyword::F64
    //             | Keyword::I128
    //             | Keyword::U128
    //             | Keyword::F128
    //             | Keyword::Sized
    //             | Keyword::Unsized
    //             | Keyword::Char
    //             | Keyword::Str
    //             | Keyword::Bool
    //             | Keyword::Nil
    //             | Keyword::BigInt
    //             | Keyword::BigFloat => return Some(kw),
    //             _ => return None,
    //         }
    //     }
    //
    //     None
    // }
}

//WARN: Not sure about the amount of casting everywhere
const TYPE_START: u32 = 0;
pub const TYPE_END: u32 = 24;

const STMT_START: u32 = 30;
const STMT_END: u32 = 32;

const SECT_START: u32 = 34;
const SECT_END: u32 = 37;

const PREDICATE_START: u32 = 38;
const PREDICATE_END: u32 = 44;

//WARN: The amount of casting here is painful. SEVERELY painful.
pub fn is_type(id: u32) -> bool {
    id <= TYPE_END
}

// But what about inheritance. What about clean code? DRY? What if I want my .chrn tree of
// dependencies where they inherit from different levels of chrn files?
// export inherit non-translucent const READ = 0b00000001
pub fn is_export(id: u32) -> bool {
    id == 29
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
