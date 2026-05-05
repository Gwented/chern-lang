use std::ops::RangeInclusive;

use common::fmter::{Formattable, Formatted};

use crate::intern;

/// Known size in bytes for `@def` and `@end`
pub const DEFINITION_SIZE: usize = 4;
//WARN: WAY TOO MANY MICRO-DEPENDENCIES

//WARN:
// Before adding a keyword:
// Ensure array string is aligned with the Keyword enum
// Ensure ranges and all keyword functions are adjusted
// Ensure tests are aligned

pub static KEYWORDS_ARRAY: [&str; 13] = [
    "struct", "enum", "import", "export", "bind", "alias", "let", "change", "as", "var", "nest",
    "complex", "override",
];
//FIX: not keywords but known identifiers
// Functions
// "Range",
// "StartsW",
// "EndsW",
// "Contains",
// "Equals",
// "Nat" // 37
// "Real" // 38
// "Complex" // 39
// "Prime" // 40

// Keep a compact enum for code that prefers typed keyword identifiers.
// I think I don't know I am new to thinking does anyone have beginner thoughts?
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[repr(u32)]
pub enum Keyword {
    Struct = 0,
    Enum = 1,
    Import = 2,
    Export = 3,
    Bind = 4,
    Alias = 5,
    Let = 6,
    Change = 7,
    As = 8,
    Var = 9,
    Nest = 10,
    Complex = 11,
    Override = 12,
}

impl Formattable for Keyword {
    fn to_fmt(&self) -> common::fmter::Formatted {
        match self {
            Keyword::Struct => Formatted::Struct,
            Keyword::Enum => Formatted::Enum,
            Keyword::Import => Formatted::Import,
            Keyword::Export => Formatted::Export,
            Keyword::Bind => Formatted::Bind,
            Keyword::Alias => Formatted::Alias,
            Keyword::Let => Formatted::Let,
            Keyword::Change => Formatted::Change,
            Keyword::Var => Formatted::SectVar,
            Keyword::Nest => Formatted::SectNest,
            Keyword::Complex => Formatted::SectComplex,
            Keyword::Override => Formatted::SectOverride,
            Keyword::As => Formatted::As,
        }
    }
}

impl Keyword {
    pub fn is_sect(&self) -> bool {
        match self {
            Keyword::Var | Keyword::Nest | Keyword::Complex | Keyword::Override => true,
            _ => false,
        }
    }

    // No. No this will not stay.
    /// Returns Some keyword that matches the given id or None
    pub fn try_from_interned_id(id: u32) -> Option<Keyword> {
        match id {
            // Using literal because scared of if
            intern::INTERNED_STRUCT => Some(Keyword::Struct),
            intern::INTERNED_ENUM => Some(Keyword::Enum),
            intern::INTERNED_IMPORT => Some(Keyword::Import),
            intern::INTERNED_EXPORT => Some(Keyword::Export),
            intern::INTERNED_BIND => Some(Keyword::Bind),
            intern::INTERNED_ALIAS => Some(Keyword::Alias),
            intern::INTERNED_LET => Some(Keyword::Let),
            intern::INTERNED_CHANGE => Some(Keyword::Change),
            intern::INTERNED_AS => Some(Keyword::As),
            intern::INTERNED_VAR => Some(Keyword::Var),
            intern::INTERNED_NEST => Some(Keyword::Nest),
            intern::INTERNED_COMPLEX => Some(Keyword::Complex),
            intern::INTERNED_OVERRIDE => Some(Keyword::Override),
            _ => None,
        }
    }

    /// Returns Some keyword that is considered a statement or None
    pub fn try_as_stmt(id: u32) -> Option<Keyword> {
        if !stmt_range().contains(&(id as usize)) {
            return None;
        }

        Keyword::try_from_interned_id(id)
    }
}

//WARN: Not sure about the amount of casting everywhere

const STMT_START: u32 = 3;
const STMT_END: u32 = 9;

pub const SECT_START: u32 = 9;
pub const SECT_END: u32 = 12;

//WARN: The amount of casting here is painful. SEVERELY painful.
pub fn is_kw(id: u32) -> bool {
    id < KEYWORDS_ARRAY.len() as u32
}

/// Ensure this aligns with the actual id of export
pub fn is_export(id: u32) -> bool {
    id == 4
}

pub fn is_sect(id: u32) -> bool {
    (SECT_START..=SECT_END).contains(&id)
}

pub fn stmt_range() -> RangeInclusive<usize> {
    (STMT_START as usize)..=(STMT_END as usize)
}

pub fn sect_range() -> RangeInclusive<usize> {
    (SECT_START as usize)..=(SECT_END as usize)
}
