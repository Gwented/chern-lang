use std::fmt::Display;

use crate::semantic::representation::FuncKind;

// Nat
// Real
// Complex
// Prime
// TEST:
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ArgConstraint {
    ArgCount(u8),
    DynType,
    MatchingType,
    Numeric,
    Integer,
    Float,
    Str,
}

impl ArgConstraint {
    // PLEASE DONT MAKE ME RETURN OPTION
    // TODO: Composable constraints for aliases
    /// Takes in a function kind that is built in and returns it's constraints
    pub fn from_builtin(kind: FuncKind) -> Vec<ArgConstraint> {
        match kind {
            //WARN: CHANGE BACK TO DYNTYPE
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
                    ArgConstraint::MatchingType,
                ]
            }
            FuncKind::UserDefined => todo!(),
        }
    }
}

impl Display for ArgConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArgConstraint::DynType => write!(f, "DynamicType"),
            ArgConstraint::MatchingType => write!(f, "MatchingType"),
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
        }
    }
}

// Can't really use AstId since it's not an ExprId and it would pretty much be a guess as to what
// typeexpr it came from
