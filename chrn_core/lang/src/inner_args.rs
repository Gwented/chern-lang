use std::fmt::Display;

use chrn_utils::source_map::source_span::SourceSpan;

use crate::types::{
    builtins::BuiltinType,
    type_constraints::{self, TypeConstraintFlags},
};

/// If a new argument is added ensure this is updated
pub static ARGS_ARRAY: [&str; 6] = ["warn", "scient", "hex", "bin", "octal", "ignore"];

#[derive(Debug, Copy, Clone)]
pub struct SpannedInnerArg {
    pub arg: InnerArgs,
    pub span: SourceSpan,
}

impl SpannedInnerArg {
    pub fn new(arg: InnerArgs, span: SourceSpan) -> SpannedInnerArg {
        SpannedInnerArg { arg, span }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InnerArgs {
    Warn,
    Scientific,
    Hex,
    Binary,
    Octal,
    Ignore,
}

impl InnerArgs {
    // has_restrictions?
    /// Returns true if the given argument is applicable to every type, such as `#warn`, otherwise
    /// returns false
    pub fn has_restrictions(self) -> bool {
        match self {
            InnerArgs::Scientific | InnerArgs::Hex | InnerArgs::Binary | InnerArgs::Octal => true,
            InnerArgs::Ignore | InnerArgs::Warn => false,
        }
    }

    /// This MUST be used after ensuring the type is a primitive, not a data structure.
    // Maybe this is a good time to use kind
    pub fn supports_builtin_type(self, builtin_type: &BuiltinType) -> bool {
        match self {
            InnerArgs::Ignore | InnerArgs::Warn => true,
            InnerArgs::Scientific | InnerArgs::Hex | InnerArgs::Binary | InnerArgs::Octal => {
                match builtin_type {
                    BuiltinType::I8
                    | BuiltinType::U8
                    | BuiltinType::I16
                    | BuiltinType::U16
                    | BuiltinType::F16
                    | BuiltinType::I32
                    | BuiltinType::U32
                    | BuiltinType::F32
                    | BuiltinType::I64
                    | BuiltinType::U64
                    | BuiltinType::F64
                    | BuiltinType::I128
                    | BuiltinType::U128
                    | BuiltinType::F128
                    | BuiltinType::Sized
                    | BuiltinType::BigInt
                    | BuiltinType::BigFloat
                    | BuiltinType::Unsized
                    //NOTE: Checks this at runtime
                    |BuiltinType::Runtime => true,
                    // Maybe this means that it shouldn't be a method, it should be a function that
                    // has access to their inner, which can do the rolving. Rolving.
                    //
                    // This is unreachable because when arguments are resolved, it requires the
                    // data structures to be recursively resolved into a builtin type
                    BuiltinType::List(_)
                    |BuiltinType::Set(_)
                    |BuiltinType::Map(_, _) => unreachable!("ConstraintResolver broke"),
                    _ => false,
                }
            }
        }
    }

    // pub fn supports_type_constraint(&self, constraint_flags: TypeConstraintFlags) -> bool {
    //     todo!()
    // match self {
    //     InnerArgs::Scientific | InnerArgs::Hex | InnerArgs::Binary | InnerArgs::Octal => {
    //         match constraint_flags.flags {
    //             TypeConstraint::Collection | TypeConstraint::HasLen if is_rec => true,
    //             TypeConstraint::Numeric
    //             | TypeConstraint::Integer
    //             | TypeConstraint::SignedInteger
    //             | TypeConstraint::UnsignedInteger
    //             | TypeConstraint::Float
    //             | TypeConstraint::Ordered
    //             | TypeConstraint::Any => true,
    //             TypeConstraint::Bool
    //             | TypeConstraint::Collection
    //             | TypeConstraint::HasLen
    //             | TypeConstraint::CharacterMappable
    //             | TypeConstraint::Char
    //             | TypeConstraint::Ranged
    //             | TypeConstraint::Comparable
    //             | TypeConstraint::Str => false,
    //         }
    //     }
    //     InnerArgs::Ignore | InnerArgs::Warn => true,
    // }
    // }

    pub fn type_constraints(self) -> TypeConstraintFlags {
        let flags = match self {
            InnerArgs::Warn | InnerArgs::Ignore => type_constraints::ALL_DOMAINS,
            InnerArgs::Scientific | InnerArgs::Hex | InnerArgs::Binary | InnerArgs::Octal => {
                type_constraints::NUMERIC
            }
        };

        TypeConstraintFlags::new(flags)
    }

    pub fn try_from_str(val: &str) -> Option<InnerArgs> {
        match val {
            "warn" => Some(InnerArgs::Warn),
            "scient" => Some(InnerArgs::Scientific),
            "hex" => Some(InnerArgs::Hex),
            "bin" => Some(InnerArgs::Binary),
            "octal" => Some(InnerArgs::Octal),
            "ignore" => Some(InnerArgs::Ignore),
            _ => None,
        }
    }
}

impl Display for InnerArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InnerArgs::Warn => write!(f, "warn"),
            InnerArgs::Scientific => write!(f, "scient"),
            InnerArgs::Hex => write!(f, "hex"),
            InnerArgs::Binary => write!(f, "bin"),
            InnerArgs::Octal => write!(f, "octal"),
            InnerArgs::Ignore => write!(f, "ignore"),
        }
    }
}
