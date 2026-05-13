use std::fmt::Display;

use chrn_utils::builtins::{BuiltinType, BuiltinTypeKind};

use crate::semantic::representation::{FuncKind, Type};

// Nat
// Real
// Complex
// Prime
// TEST:
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgConstraint {
    ArgCount(u32),
    DynType,
    MatchingArgumentTypes,
    /// Must be the same type as the type the condition is made for
    MirroredType,
    Numeric,
    Integer,
    Float,
    CharacterMappable,
    Bool,
    Str,
    // Suspicious
    Variadic,
}

impl ArgConstraint {
    // PLEASE DONT MAKE ME RETURN OPTION
    // TODO: Composable constraints for aliases
    /// Takes in a function kind that is built in and returns it's constraints
    pub fn from_builtin(kind: FuncKind) -> Vec<ArgConstraint> {
        match kind {
            FuncKind::IsEmpty => vec![ArgConstraint::ArgCount(1)],
            FuncKind::StartsW => {
                // Maybe if we got something like 0x1FF it could StartsW(0x1FF)?
                vec![ArgConstraint::ArgCount(1), ArgConstraint::DynType]
            }
            FuncKind::EndsW => {
                vec![ArgConstraint::ArgCount(1), ArgConstraint::DynType]
            }
            FuncKind::Contains => {
                vec![ArgConstraint::ArgCount(1), ArgConstraint::DynType]
            }
            FuncKind::Range => {
                vec![
                    ArgConstraint::ArgCount(2),
                    ArgConstraint::Numeric,
                    ArgConstraint::MatchingArgumentTypes,
                ]
            }
            FuncKind::Equals => {
                vec![ArgConstraint::MirroredType, ArgConstraint::Variadic]
            }
            FuncKind::IsWhitespace => vec![ArgConstraint::ArgCount(1), ArgConstraint::Str],
        }
    }
}

impl Display for ArgConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArgConstraint::DynType => write!(f, "DynamicType"),
            ArgConstraint::MatchingArgumentTypes => write!(f, "MatchingType"),
            ArgConstraint::Numeric => write!(f, "Numeric"),
            ArgConstraint::Integer => write!(f, "Integer"),
            ArgConstraint::Float => write!(f, "Float"),
            ArgConstraint::Str => write!(f, "str"),
            ArgConstraint::ArgCount(count) => {
                if *count > 1 {
                    write!(f, "{count} arguments")
                } else {
                    write!(f, "{count} argument")
                }
            }
            ArgConstraint::MirroredType => write!(f, "MirroredType"),
            ArgConstraint::CharacterMappable => write!(f, "CharacterMappable"),
            ArgConstraint::Variadic => write!(f, "variadic"),
            ArgConstraint::Bool => write!(f, "bool"),
        }
    }
}

// Can't really use AstId since it's not an ExprId and it would pretty much be a guess as to what
// typeexpr it came from
