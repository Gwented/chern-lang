use std::fmt::Display;

use common::span::Span;

use crate::builtins::BuiltinType;

/// If a new argument is added ensure this is updated
pub static ARGS_ARRAY: [&str; 6] = ["warn", "scient", "hex", "bin", "octal", "ignore"];

#[derive(Debug, Clone)]
pub struct SpannedInnerArgs {
    pub arg: InnerArgs,
    pub span: Span,
}

impl SpannedInnerArgs {
    pub fn new(arg: InnerArgs, span: Span) -> SpannedInnerArgs {
        SpannedInnerArgs { arg, span }
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
    pub fn has_restrictions(&self) -> bool {
        match self {
            InnerArgs::Scientific | InnerArgs::Hex | InnerArgs::Binary | InnerArgs::Octal => true,
            InnerArgs::Ignore | InnerArgs::Warn => false,
        }
    }

    /// This MUST be used after ensuring the type is a primitive, not a data structure.
    // Maybe this is a good time to use kind
    pub fn supports_builtin_type(&self, builtin_type: &BuiltinType) -> bool {
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
                    |BuiltinType::Any => true,
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

//TODO: Should be some or none
impl<'a> TryFrom<&'a str> for InnerArgs {
    type Error = &'a str;

    fn try_from(v: &'a str) -> Result<Self, Self::Error> {
        match v {
            "warn" => Ok(InnerArgs::Warn),
            "scient" => Ok(InnerArgs::Scientific),
            "hex" => Ok(InnerArgs::Hex),
            "bin" => Ok(InnerArgs::Binary),
            "octal" => Ok(InnerArgs::Octal),
            "ignore" => Ok(InnerArgs::Ignore),
            v => Err(v),
        }
    }
}
