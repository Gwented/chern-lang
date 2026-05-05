use common::fmter::{Formattable, Formatted};

use crate::{id_types::TypeId, intern};

pub static BUILTIN_TYPE_ARRAY: [&str; 27] = [
    "i8", "u8", "i16", "u16", "f16", "i32", "u32", "f32", "i64", "u64", "f64", "i128", "u128",
    "f128", "sized", "unsized", "char", "str", "bool", "nil", "BigInt", "BigFloat", "List", "Map",
    "Set", "Tuple", "any",
];

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
    Map(TypeId, TypeId),
    Set(TypeId),
    Tuple(Vec<TypeId>),
    Any,
}

impl BuiltinType {
    /// Returns the corresponding builtin type from a pre-determined interned id (Excluding data
    /// structures)
    pub fn try_from_interned_id(id: u32) -> Option<BuiltinType> {
        match id {
            intern::INTERNED_I8 => Some(BuiltinType::I8),
            intern::INTERNED_U8 => Some(BuiltinType::U8),
            intern::INTERNED_I16 => Some(BuiltinType::I16),
            intern::INTERNED_U16 => Some(BuiltinType::U16),
            intern::INTERNED_F16 => Some(BuiltinType::F16),
            intern::INTERNED_I32 => Some(BuiltinType::I32),
            intern::INTERNED_U32 => Some(BuiltinType::U32),
            intern::INTERNED_F32 => Some(BuiltinType::F32),
            intern::INTERNED_I64 => Some(BuiltinType::I64),
            intern::INTERNED_U64 => Some(BuiltinType::U64),
            intern::INTERNED_F64 => Some(BuiltinType::F64),
            intern::INTERNED_I128 => Some(BuiltinType::I128),
            intern::INTERNED_U128 => Some(BuiltinType::U128),
            intern::INTERNED_F128 => Some(BuiltinType::F128),
            intern::INTERNED_SIZED => Some(BuiltinType::Sized),
            intern::INTERNED_UNSIZED => Some(BuiltinType::Unsized),
            intern::INTERNED_BOOL => Some(BuiltinType::Bool),
            intern::INTERNED_NIL => Some(BuiltinType::Nil),
            intern::INTERNED_CHAR => Some(BuiltinType::Char),
            intern::INTERNED_STR => Some(BuiltinType::Str),
            intern::INTERNED_BIGINT => Some(BuiltinType::BigInt),
            intern::INTERNED_BIGFLOAT => Some(BuiltinType::BigFloat),
            intern::INTERNED_ANY => Some(BuiltinType::Any),
            _ => None,
        }
    }

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
            BuiltinType::Any => BuiltinTypeKind::Any,
            BuiltinType::List(_) => BuiltinTypeKind::List,
            BuiltinType::Set(_) => BuiltinTypeKind::Set,
            BuiltinType::Map(_, _) => BuiltinTypeKind::Map,
            BuiltinType::Tuple(_) => BuiltinTypeKind::Tuple,
        }
    }
}

// TODO: Something.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum BuiltinTypeKind {
    I8 = 0,
    U8 = 1,
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
    Tuple,
    Any,
}

impl BuiltinTypeKind {
    pub fn try_from_interned_id(id: u32) -> Option<BuiltinTypeKind> {
        match id {
            intern::INTERNED_I8 => Some(BuiltinTypeKind::I8),
            intern::INTERNED_U8 => Some(BuiltinTypeKind::U8),
            intern::INTERNED_I16 => Some(BuiltinTypeKind::I16),
            intern::INTERNED_U16 => Some(BuiltinTypeKind::U16),
            intern::INTERNED_F16 => Some(BuiltinTypeKind::F16),
            intern::INTERNED_I32 => Some(BuiltinTypeKind::I32),
            intern::INTERNED_U32 => Some(BuiltinTypeKind::U32),
            intern::INTERNED_F32 => Some(BuiltinTypeKind::F32),
            intern::INTERNED_I64 => Some(BuiltinTypeKind::I64),
            intern::INTERNED_U64 => Some(BuiltinTypeKind::U64),
            intern::INTERNED_F64 => Some(BuiltinTypeKind::F64),
            intern::INTERNED_I128 => Some(BuiltinTypeKind::I128),
            intern::INTERNED_U128 => Some(BuiltinTypeKind::U128),
            intern::INTERNED_F128 => Some(BuiltinTypeKind::F128),
            intern::INTERNED_SIZED => Some(BuiltinTypeKind::Sized),
            intern::INTERNED_UNSIZED => Some(BuiltinTypeKind::Unsized),
            intern::INTERNED_BOOL => Some(BuiltinTypeKind::Bool),
            intern::INTERNED_NIL => Some(BuiltinTypeKind::Nil),
            intern::INTERNED_CHAR => Some(BuiltinTypeKind::Char),
            intern::INTERNED_STR => Some(BuiltinTypeKind::Str),
            intern::INTERNED_BIGINT => Some(BuiltinTypeKind::BigInt),
            intern::INTERNED_BIGFLOAT => Some(BuiltinTypeKind::BigFloat),
            intern::INTERNED_LIST => Some(BuiltinTypeKind::List),
            intern::INTERNED_MAP => Some(BuiltinTypeKind::Map),
            intern::INTERNED_SET => Some(BuiltinTypeKind::Set),
            intern::INTERNED_TUPLE => Some(BuiltinTypeKind::Tuple),
            _ => None,
        }
    }
}

impl Formattable for BuiltinTypeKind {
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
            BuiltinTypeKind::Tuple => Formatted::Tuple,
        }
    }
}

// SHOULD THIS ERR?
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
            | BuiltinTypeKind::Tuple
            | BuiltinTypeKind::Any => false,
        }
    }
}
