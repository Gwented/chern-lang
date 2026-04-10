use common::{
    builtins::BuiltinTypeKind,
    fmter::{Formattable, Formatted},
    keywords::Keyword,
    symbols::{Span, SymbolId},
};

use crate::{semantic::representation::FuncKind, types::token::Token};

#[derive(Debug, Clone)]
pub struct SpannedToken {
    pub(crate) tok: Token,
    pub span: Span,
}

// Spanned cond can't exist because conds are expressions
#[derive(Debug, Clone)]
pub enum Cond {
    //FIX:
    Func(SymbolId, FuncKind),
    IsEmpty,
    IsWhitespace,
    Not(Box<Cond>),
}

impl Formattable for Cond {
    fn to_fmt(&self) -> common::fmter::Formatted {
        match self {
            Cond::IsEmpty => Formatted::IsEmpty,
            Cond::IsWhitespace => Formatted::IsWhitespace,
            Cond::Not(cond) => Formatted::Nothing,
            // Maybe pair with kind?
            Cond::Func(_, kind) => kind.to_fmt(),
        }
    }
}

// I'm actually fine with this.
impl Cond {
    /// Only returns a condition if it is solely a keyword, excluding any functional
    /// conditions.
    // This is really really really really smelly
    pub fn try_from_id(id: u32) -> Option<Cond> {
        match Keyword::try_as_kw(id) {
            Some(kw) => match kw {
                Keyword::IsEmpty => Some(Cond::IsEmpty),
                Keyword::IsWhitespace => Some(Cond::IsWhitespace),
                _ => None,
            },
            None => None,
        }
    }

    pub fn try_from_kw(kw: Keyword) -> Option<Cond> {
        match kw {
            Keyword::IsEmpty => Some(Cond::IsEmpty),
            Keyword::IsWhitespace => Some(Cond::IsWhitespace),
            _ => None,
        }
    }

    pub fn supports_builtin_type(&self, kind: BuiltinTypeKind) -> bool {
        match self {
            Cond::IsEmpty | Cond::IsWhitespace => {
                if kind.is_numeric() {
                    return false;
                }

                true
            }
            Cond::Not(cond) => Self::supports_builtin_type(cond, kind),
            Cond::Func(_, _) => unreachable!("Not a possible variant"),
        }
    }
}
