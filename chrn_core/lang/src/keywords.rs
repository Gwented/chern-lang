use std::ops::RangeInclusive;

use chrn_utils::{id_types::InternedId, intern};

use crate::fmter::{self, Formattable, Formatted};

/// Size in bytes for `@def` and `@end`
pub const ANNOTATION_CLAUSE_SIZE: usize = 4;

pub const DEF_CLAUSE_BYTES: &[u8; 4] = b"@def";
pub const END_CLAUSE_BYTES: &[u8; 4] = b"@end";

// To add a keyword, it must be added as a Keyword enum. The interner must intern it's identifier.
/// All keywords for `chrn`
pub static KEYWORDS_ARRAY: [&str; 14] = [
    "struct", "enum", "import", "export", "bind", "alias", "let", "change", "as", "in", "var",
    "nest", "complex", "override",
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
    In = 13,
}

impl Formattable for Keyword {
    fn to_fmt(&self) -> fmter::Formatted {
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
            Keyword::In => Formatted::In,
        }
    }
}

impl Keyword {
    pub fn name_id(self) -> InternedId {
        let id = match self {
            Keyword::Struct => intern::INTERNED_STRUCT,
            Keyword::Enum => intern::INTERNED_ENUM,
            Keyword::Import => intern::INTERNED_IMPORT,
            Keyword::Export => intern::INTERNED_EXPORT,
            Keyword::Bind => intern::INTERNED_BIND,
            Keyword::Alias => intern::INTERNED_ALIAS,
            Keyword::Let => intern::INTERNED_LET,
            Keyword::Change => intern::INTERNED_CHANGE,
            Keyword::As => intern::INTERNED_AS,
            Keyword::Var => intern::INTERNED_VAR,
            Keyword::Nest => intern::INTERNED_NEST,
            Keyword::Complex => intern::INTERNED_COMPLEX,
            Keyword::Override => intern::INTERNED_OVERRIDE,
            Keyword::In => intern::INTERNED_IN,
        };
        InternedId::new(id)
    }

    pub fn is_sect(self) -> bool {
        match self {
            Keyword::Var | Keyword::Nest | Keyword::Complex | Keyword::Override => true,
            _ => false,
        }
    }

    // No. No this will not stay.
    /// Returns Some keyword that matches the given id or None
    pub fn try_from_interned_id(interned_id: InternedId) -> Option<Keyword> {
        match interned_id.id {
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
            intern::INTERNED_IN => Some(Keyword::In),
            _ => None,
        }
    }

    /// Returns Some keyword that is considered a statement or None
    pub fn try_as_stmt(interned_id: InternedId) -> Option<Keyword> {
        if !stmt_range().contains(&(interned_id.id as usize)) {
            return None;
        }
        Keyword::try_from_interned_id(interned_id)
    }
}

//WARN: Not sure about the amount of casting everywhere

const STMT_START: u32 = 3;
const STMT_END: u32 = 9;

pub const SECT_START: u32 = 9;
pub const SECT_END: u32 = 12;

pub fn is_sect(id: u32) -> bool {
    (SECT_START..=SECT_END).contains(&id)
}

pub fn stmt_range() -> RangeInclusive<usize> {
    (STMT_START as usize)..=(STMT_END as usize)
}

pub fn sect_range() -> RangeInclusive<usize> {
    (SECT_START as usize)..=(SECT_END as usize)
}
