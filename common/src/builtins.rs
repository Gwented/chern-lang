use std::fmt::Display;

use crate::{
    fmter::{Formatable, Formatted},
    keywords::Keyword,
    symbols::TypeId,
};

//TEST: Serial and script interact with this directly so
#[derive(Debug)]
pub enum BuiltinType {
    I8,
    U8,
    I16,
    U16,
    F16,
    I32,
    U32,
    F32,
    I64,
    U64,
    F64,
    I128,
    U128,
    F128,
    Sized,
    Unsized,
    Bool,
    Nil,
    Char,
    Str,
    BigInt,
    BigFloat,
    List(TypeId),
    Set(TypeId),
    Map(TypeId, TypeId),
    Any(Option<TypeId>),
}

impl BuiltinType {
    pub fn kind(&self) -> BuiltinTypeKind {
        match self {
            BuiltinType::I8 => BuiltinTypeKind::I8,
            BuiltinType::U8 => BuiltinTypeKind::U8,
            BuiltinType::I16 => BuiltinTypeKind::I16,
            BuiltinType::U16 => BuiltinTypeKind::U16,
            BuiltinType::F16 => BuiltinTypeKind::F16,
            BuiltinType::I32 => BuiltinTypeKind::I32,
            BuiltinType::U32 => BuiltinTypeKind::U32,
            BuiltinType::F32 => BuiltinTypeKind::F32,
            BuiltinType::I64 => BuiltinTypeKind::I64,
            BuiltinType::U64 => BuiltinTypeKind::U64,
            BuiltinType::F64 => BuiltinTypeKind::F64,
            BuiltinType::I128 => BuiltinTypeKind::I128,
            BuiltinType::U128 => BuiltinTypeKind::U128,
            BuiltinType::F128 => BuiltinTypeKind::F128,
            BuiltinType::Sized => BuiltinTypeKind::Sized,
            BuiltinType::Unsized => BuiltinTypeKind::Unsized,
            BuiltinType::Bool => BuiltinTypeKind::Bool,
            BuiltinType::Nil => BuiltinTypeKind::Nil,
            BuiltinType::Char => BuiltinTypeKind::Char,
            BuiltinType::Str => BuiltinTypeKind::Str,
            BuiltinType::BigInt => BuiltinTypeKind::BigInt,
            BuiltinType::BigFloat => BuiltinTypeKind::BigFloat,
            BuiltinType::List(_) => BuiltinTypeKind::List,
            BuiltinType::Set(_) => BuiltinTypeKind::Set,
            BuiltinType::Map(_, _) => BuiltinTypeKind::Map,
            BuiltinType::Any(_) => BuiltinTypeKind::Any,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinTypeKind {
    I8,
    U8,
    I16,
    U16,
    F16,
    I32,
    U32,
    F32,
    I64,
    U64,
    F64,
    I128,
    U128,
    F128,
    Sized,
    Unsized,
    Str,
    Char,
    Nil,
    Bool,
    BigInt,
    BigFloat,
    List,
    Set,
    Map,
    Any,
}

impl Formatable for BuiltinTypeKind {
    fn to_fmt(&self) -> Formatted {
        match self {
            BuiltinTypeKind::I8 => Formatted::I8,
            BuiltinTypeKind::U8 => Formatted::U8,
            BuiltinTypeKind::I16 => Formatted::I16,
            BuiltinTypeKind::U16 => Formatted::U16,
            BuiltinTypeKind::F16 => Formatted::F16,
            BuiltinTypeKind::I32 => Formatted::I32,
            BuiltinTypeKind::U32 => Formatted::U32,
            BuiltinTypeKind::F32 => Formatted::F32,
            BuiltinTypeKind::I64 => Formatted::I64,
            BuiltinTypeKind::U64 => Formatted::U64,
            BuiltinTypeKind::F64 => Formatted::F64,
            BuiltinTypeKind::I128 => Formatted::I128,
            BuiltinTypeKind::U128 => Formatted::U128,
            BuiltinTypeKind::F128 => Formatted::F128,
            BuiltinTypeKind::Sized => Formatted::Sized,
            BuiltinTypeKind::Unsized => Formatted::Unsized,
            BuiltinTypeKind::Str => Formatted::Str,
            BuiltinTypeKind::Char => Formatted::Char,
            BuiltinTypeKind::Nil => Formatted::Nil,
            BuiltinTypeKind::Bool => Formatted::Bool,
            BuiltinTypeKind::BigInt => Formatted::BigInt,
            BuiltinTypeKind::BigFloat => Formatted::BigFloat,
            BuiltinTypeKind::List => Formatted::List,
            BuiltinTypeKind::Set => Formatted::Set,
            BuiltinTypeKind::Map => Formatted::Map,
            BuiltinTypeKind::Any => Formatted::Any,
        }
    }
}

// SHOULD THIS ERR?
impl BuiltinType {
    //TODO: Find out if one of these should be removed

    /// Uses `Keyword` to map directly to a `BuiltinType` excluding data structures.
    pub fn try_from_kw(kw: Keyword) -> Option<BuiltinType> {
        match kw {
            Keyword::I8 => Some(BuiltinType::I8),
            Keyword::U8 => Some(BuiltinType::U8),
            Keyword::I16 => Some(BuiltinType::I16),
            Keyword::U16 => Some(BuiltinType::U16),
            Keyword::F16 => Some(BuiltinType::F16),
            Keyword::I32 => Some(BuiltinType::I32),
            Keyword::U32 => Some(BuiltinType::U32),
            Keyword::F32 => Some(BuiltinType::F32),
            Keyword::I64 => Some(BuiltinType::I64),
            Keyword::U64 => Some(BuiltinType::U64),
            Keyword::F64 => Some(BuiltinType::F64),
            Keyword::I128 => Some(BuiltinType::I128),
            Keyword::U128 => Some(BuiltinType::U128),
            Keyword::F128 => Some(BuiltinType::F128),
            Keyword::Sized => Some(BuiltinType::Sized),
            Keyword::Unsized => Some(BuiltinType::Unsized),
            Keyword::Char => Some(BuiltinType::Char),
            Keyword::Str => Some(BuiltinType::Str),
            Keyword::Bool => Some(BuiltinType::Bool),
            Keyword::Nil => Some(BuiltinType::Nil),
            Keyword::BigInt => Some(BuiltinType::BigInt),
            Keyword::BigFloat => Some(BuiltinType::BigFloat),
            _ => None,
        }
    }

    //NOTE: This may still be replaced by a `BuiltinType` TypeExpr but seems fine
    /// Uses `Keyword` to map directly to a `BuiltinType` excluding data structures.
    pub fn try_from_id(id: u32) -> Option<BuiltinType> {
        match Keyword::try_as_kw(id) {
            Some(kw) => match kw {
                Keyword::I8 => Some(BuiltinType::I8),
                Keyword::U8 => Some(BuiltinType::U8),
                Keyword::I16 => Some(BuiltinType::I16),
                Keyword::U16 => Some(BuiltinType::U16),
                Keyword::F16 => Some(BuiltinType::F16),
                Keyword::I32 => Some(BuiltinType::I32),
                Keyword::U32 => Some(BuiltinType::U32),
                Keyword::F32 => Some(BuiltinType::F32),
                Keyword::I64 => Some(BuiltinType::I64),
                Keyword::U64 => Some(BuiltinType::U64),
                Keyword::F64 => Some(BuiltinType::F64),
                Keyword::I128 => Some(BuiltinType::I128),
                Keyword::U128 => Some(BuiltinType::U128),
                Keyword::F128 => Some(BuiltinType::F128),
                Keyword::Sized => Some(BuiltinType::Sized),
                Keyword::Unsized => Some(BuiltinType::Unsized),
                Keyword::Char => Some(BuiltinType::Char),
                Keyword::Str => Some(BuiltinType::Str),
                Keyword::Bool => Some(BuiltinType::Bool),
                Keyword::Nil => Some(BuiltinType::Nil),
                Keyword::BigInt => Some(BuiltinType::BigInt),
                Keyword::BigFloat => Some(BuiltinType::BigFloat),
                _ => None,
            },
            None => None,
        }
    }
}

impl BuiltinTypeKind {
    pub fn is_numeric(&self) -> bool {
        match self {
            BuiltinTypeKind::I8
            | BuiltinTypeKind::U8
            | BuiltinTypeKind::I16
            | BuiltinTypeKind::U16
            | BuiltinTypeKind::F16
            | BuiltinTypeKind::I32
            | BuiltinTypeKind::U32
            | BuiltinTypeKind::F32
            | BuiltinTypeKind::I64
            | BuiltinTypeKind::U64
            | BuiltinTypeKind::F64
            | BuiltinTypeKind::I128
            | BuiltinTypeKind::U128
            | BuiltinTypeKind::F128
            | BuiltinTypeKind::Sized
            | BuiltinTypeKind::Unsized
            | BuiltinTypeKind::BigInt
            | BuiltinTypeKind::BigFloat => true,
            BuiltinTypeKind::Bool
            | BuiltinTypeKind::Nil
            | BuiltinTypeKind::Char
            | BuiltinTypeKind::Str
            | BuiltinTypeKind::List
            | BuiltinTypeKind::Set
            | BuiltinTypeKind::Map
            | BuiltinTypeKind::Any => false,
        }
    }
}

//TEST:
impl Display for BuiltinTypeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuiltinTypeKind::I8 => write!(f, "i8"),
            BuiltinTypeKind::U8 => write!(f, "u8"),
            BuiltinTypeKind::I16 => write!(f, "u16"),
            BuiltinTypeKind::U16 => write!(f, "u16"),
            BuiltinTypeKind::F16 => write!(f, "f16"),
            BuiltinTypeKind::I32 => write!(f, "i32"),
            BuiltinTypeKind::U32 => write!(f, "u32"),
            BuiltinTypeKind::F32 => write!(f, "f32"),
            BuiltinTypeKind::I64 => write!(f, "i64"),
            BuiltinTypeKind::U64 => write!(f, "u64"),
            BuiltinTypeKind::F64 => write!(f, "f64"),
            BuiltinTypeKind::I128 => write!(f, "i128"),
            BuiltinTypeKind::U128 => write!(f, "u128"),
            BuiltinTypeKind::F128 => write!(f, "f128"),
            BuiltinTypeKind::Sized => write!(f, "sized"),
            BuiltinTypeKind::Unsized => write!(f, "unsized"),
            BuiltinTypeKind::Str => write!(f, "str"),
            BuiltinTypeKind::Char => write!(f, "char"),
            BuiltinTypeKind::Nil => write!(f, "nil"),
            BuiltinTypeKind::Bool => write!(f, "bool"),
            BuiltinTypeKind::BigInt => write!(f, "BigInt"),
            BuiltinTypeKind::BigFloat => write!(f, "BigFloat"),
            BuiltinTypeKind::List => write!(f, "List"),
            BuiltinTypeKind::Set => write!(f, "Set"),
            BuiltinTypeKind::Map => write!(f, "Map"),
            BuiltinTypeKind::Any => write!(f, "Any"),
        }
    }
}
