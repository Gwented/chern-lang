use chrn_utils::{builtins::BuiltinTypeKind, id_types::SymbolId, intern, keywords::Keyword};
use common::fmter::{Formattable, Formatted};

use crate::semantic::representation::FuncKind;

#[derive(Debug, Clone)]
pub enum Cond {
    //FIX:
    Func(SymbolId, FuncKind),
    IsEmpty,
    IsWhitespace,
    // Predicate(ExprId),
    Not(Box<Cond>),
}

impl Formattable for Cond {
    fn to_fmt(&self) -> common::fmter::Formatted {
        match self {
            Cond::IsEmpty => Formatted::IsEmpty,
            Cond::IsWhitespace => Formatted::IsWhitespace,
            Cond::Not(cond) => Formatted::Unknown,
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
    pub fn try_from_interned_id(id: u32) -> Option<Cond> {
        match id {
            intern::INTERNED_IS_EMPTY => Some(Cond::IsEmpty),
            intern::INTERNED_IS_WHITESPACE => Some(Cond::IsWhitespace),
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
