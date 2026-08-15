use chrn_utils::{
    id_types::{InternedId, TypeId},
    intern,
};

use crate::{
    chrn_classifier::{ChrnClassifiable, ChrnClassifier},
    types::boundaries::TypeBoundaryFlags,
};

pub static BUILTIN_TYPE_ARRAY: [&str; 27] = [
    "i8", "u8", "i16", "u16", "f16", "i32", "u32", "f32", "i64", "u64", "f64", "i128", "u128",
    "f128", "sized", "unsized", "char", "str", "bool", "nil", "BigInt", "BigFloat", "List", "Map",
    "Set", "Tuple", "Runtime",
];

#[derive(Debug, Clone)]
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
    //TODO: any should either disallow all conditions and only take in unrestricted arguments, or
    //be type inferred, given arguments or conditions
    Runtime,
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
            intern::INTERNED_RUNTIME => Some(BuiltinType::Runtime),
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
            BuiltinType::Runtime => BuiltinTypeKind::Runtime,
            BuiltinType::List(_) => BuiltinTypeKind::List,
            BuiltinType::Set(_) => BuiltinTypeKind::Set,
            BuiltinType::Map(_, _) => BuiltinTypeKind::Map,
            BuiltinType::Tuple(_) => BuiltinTypeKind::Tuple,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    Runtime,
}

impl ChrnClassifiable for BuiltinTypeKind {
    fn to_classified(&self) -> ChrnClassifier {
        match self {
            BuiltinTypeKind::I8 => ChrnClassifier::I8,
            BuiltinTypeKind::U8 => ChrnClassifier::U8,
            BuiltinTypeKind::I16 => ChrnClassifier::I16,
            BuiltinTypeKind::U16 => ChrnClassifier::U16,
            BuiltinTypeKind::F16 => ChrnClassifier::F16,
            BuiltinTypeKind::I32 => ChrnClassifier::I32,
            BuiltinTypeKind::U32 => ChrnClassifier::U32,
            BuiltinTypeKind::F32 => ChrnClassifier::F32,
            BuiltinTypeKind::I64 => ChrnClassifier::I64,
            BuiltinTypeKind::U64 => ChrnClassifier::U64,
            BuiltinTypeKind::F64 => ChrnClassifier::F64,
            BuiltinTypeKind::I128 => ChrnClassifier::I128,
            BuiltinTypeKind::U128 => ChrnClassifier::U128,
            BuiltinTypeKind::F128 => ChrnClassifier::F128,
            BuiltinTypeKind::Sized => ChrnClassifier::Sized,
            BuiltinTypeKind::Unsized => ChrnClassifier::Unsized,
            BuiltinTypeKind::Str => ChrnClassifier::Str,
            BuiltinTypeKind::Char => ChrnClassifier::Char,
            BuiltinTypeKind::Nil => ChrnClassifier::Nil,
            BuiltinTypeKind::Bool => ChrnClassifier::Bool,
            BuiltinTypeKind::BigInt => ChrnClassifier::BigInt,
            BuiltinTypeKind::BigFloat => ChrnClassifier::BigFloat,
            BuiltinTypeKind::List => ChrnClassifier::List,
            BuiltinTypeKind::Set => ChrnClassifier::Set,
            BuiltinTypeKind::Map => ChrnClassifier::Map,
            BuiltinTypeKind::Runtime => ChrnClassifier::Runtime,
            BuiltinTypeKind::Tuple => ChrnClassifier::Tuple,
        }
    }
}

impl BuiltinTypeKind {
    pub fn name_id(self) -> InternedId {
        let id = match self {
            BuiltinTypeKind::I8 => intern::INTERNED_I8,
            BuiltinTypeKind::U8 => intern::INTERNED_U8,
            BuiltinTypeKind::I16 => intern::INTERNED_I16,
            BuiltinTypeKind::U16 => intern::INTERNED_U16,
            BuiltinTypeKind::F16 => intern::INTERNED_F16,
            BuiltinTypeKind::I32 => intern::INTERNED_I32,
            BuiltinTypeKind::U32 => intern::INTERNED_U32,
            BuiltinTypeKind::F32 => intern::INTERNED_F32,
            BuiltinTypeKind::I64 => intern::INTERNED_I64,
            BuiltinTypeKind::U64 => intern::INTERNED_U64,
            BuiltinTypeKind::F64 => intern::INTERNED_F64,
            BuiltinTypeKind::I128 => intern::INTERNED_I128,
            BuiltinTypeKind::U128 => intern::INTERNED_U128,
            BuiltinTypeKind::F128 => intern::INTERNED_F128,
            BuiltinTypeKind::Sized => intern::INTERNED_SIZED,
            BuiltinTypeKind::Unsized => intern::INTERNED_UNSIZED,
            BuiltinTypeKind::Str => intern::INTERNED_STR,
            BuiltinTypeKind::Char => intern::INTERNED_CHAR,
            BuiltinTypeKind::Nil => intern::INTERNED_NIL,
            BuiltinTypeKind::Bool => intern::INTERNED_BOOL,
            BuiltinTypeKind::BigInt => intern::INTERNED_BIGINT,
            BuiltinTypeKind::BigFloat => intern::INTERNED_BIGFLOAT,
            BuiltinTypeKind::List => intern::INTERNED_LIST,
            BuiltinTypeKind::Set => intern::INTERNED_SET,
            BuiltinTypeKind::Map => intern::INTERNED_MAP,
            BuiltinTypeKind::Tuple => intern::INTERNED_TUPLE,
            BuiltinTypeKind::Runtime => intern::INTERNED_RUNTIME,
        };
        InternedId::new(id)
    }

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

    //NOTE: UPDATE WHEN NEW CONSTRAINT IS MADE
    //No
    /// Retrieves non-recursive constraints associated with type
    pub fn boundaries(self) -> TypeBoundaryFlags {
        match self {
            BuiltinTypeKind::I8
            | BuiltinTypeKind::I16
            | BuiltinTypeKind::I32
            | BuiltinTypeKind::I64
            | BuiltinTypeKind::I128
            | BuiltinTypeKind::Sized
            | BuiltinTypeKind::BigInt => {
                TypeBoundaryFlags::SIGNED_INTEGER
                    | TypeBoundaryFlags::INTEGER
                    | TypeBoundaryFlags::RANGED
                    | TypeBoundaryFlags::NUMERIC
                    | TypeBoundaryFlags::COMPARABLE
            }
            BuiltinTypeKind::U8
            | BuiltinTypeKind::U16
            | BuiltinTypeKind::U32
            | BuiltinTypeKind::U64
            | BuiltinTypeKind::U128
            | BuiltinTypeKind::Unsized => {
                TypeBoundaryFlags::UNSIGNED_INTEGER
                    | TypeBoundaryFlags::INTEGER
                    | TypeBoundaryFlags::RANGED
                    | TypeBoundaryFlags::NUMERIC
                    | TypeBoundaryFlags::COMPARABLE
            }
            BuiltinTypeKind::F16
            | BuiltinTypeKind::F32
            | BuiltinTypeKind::F64
            | BuiltinTypeKind::F128
            | BuiltinTypeKind::BigFloat => {
                TypeBoundaryFlags::FLOAT
                    | TypeBoundaryFlags::RANGED
                    | TypeBoundaryFlags::NUMERIC
                    | TypeBoundaryFlags::COMPARABLE
            }
            BuiltinTypeKind::Str => {
                TypeBoundaryFlags::STR
                    | TypeBoundaryFlags::HAS_LEN
                    | TypeBoundaryFlags::RANGED
                    | TypeBoundaryFlags::COMPARABLE
                    | TypeBoundaryFlags::CHARACTER_MAPPABLE
            }
            BuiltinTypeKind::Char => {
                TypeBoundaryFlags::CHAR
                    | TypeBoundaryFlags::HAS_LEN
                    | TypeBoundaryFlags::RANGED
                    | TypeBoundaryFlags::COMPARABLE
                    | TypeBoundaryFlags::CHARACTER_MAPPABLE
            }
            BuiltinTypeKind::Bool => TypeBoundaryFlags::COMPARABLE,
            // "non-recursive" because may allow recursive finding of inner of list if labeled
            // explicitly but not sure
            BuiltinTypeKind::List | BuiltinTypeKind::Set | BuiltinTypeKind::Tuple => {
                TypeBoundaryFlags::HAS_LEN
                    | TypeBoundaryFlags::RANGED
                    // | type_constraints::COMPARABLE
                    | TypeBoundaryFlags::CHARACTER_MAPPABLE
                    | TypeBoundaryFlags::COLLECTION
            }
            BuiltinTypeKind::Map => {
                TypeBoundaryFlags::HAS_LEN
                    | TypeBoundaryFlags::RANGED
                    // | type_constraints::COMPARABLE
                    | TypeBoundaryFlags::COLLECTION
            }
            // Maybe make this a 0
            // type_constraints::SIGNED_INTEGER
            //     | type_constraints::UNSIGNED_INTEGER
            //     | type_constraints::FLOAT
            //     | type_constraints::BOOL
            //     | type_constraints::STR
            //     | type_constraints::CHAR
            //     | type_constraints::ANY
            //     | type_constraints::COMPARABLE
            //     | type_constraints::CHARACTER_MAPPABLE
            //     | type_constraints::HAS_LEN
            //     | type_constraints::INTEGER
            //     | type_constraints::NUMERIC
            //     | type_constraints::RANGED
            //     | type_constraints::COLLECTION
            //     | type_constraints::ORDERED
            // Not sure about this
            BuiltinTypeKind::Nil => TypeBoundaryFlags::NIL,
            BuiltinTypeKind::Runtime => TypeBoundaryFlags::RUNTIME,
        }
    }

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
            | BuiltinTypeKind::BigFloat
            | BuiltinTypeKind::Runtime => true,
            BuiltinTypeKind::Bool
            | BuiltinTypeKind::Nil
            | BuiltinTypeKind::Char
            | BuiltinTypeKind::Str
            | BuiltinTypeKind::List
            | BuiltinTypeKind::Set
            | BuiltinTypeKind::Map
            | BuiltinTypeKind::Tuple => false,
        }
    }

    pub fn is_integer(&self) -> bool {
        match self {
            BuiltinTypeKind::I8
            | BuiltinTypeKind::U8
            | BuiltinTypeKind::I16
            | BuiltinTypeKind::U16
            | BuiltinTypeKind::I32
            | BuiltinTypeKind::U32
            | BuiltinTypeKind::I64
            | BuiltinTypeKind::U64
            | BuiltinTypeKind::I128
            | BuiltinTypeKind::U128
            | BuiltinTypeKind::Sized
            | BuiltinTypeKind::Unsized
            | BuiltinTypeKind::BigInt
            | BuiltinTypeKind::Runtime => true,
            _ => false,
        }
    }

    pub fn is_signed_integer(&self) -> bool {
        match self {
            BuiltinTypeKind::I8
            | BuiltinTypeKind::I16
            | BuiltinTypeKind::I32
            | BuiltinTypeKind::I64
            | BuiltinTypeKind::I128
            | BuiltinTypeKind::Sized
            | BuiltinTypeKind::BigInt
            | BuiltinTypeKind::Runtime => true,
            _ => false,
        }
    }

    pub fn is_unsigned_integer(&self) -> bool {
        match self {
            BuiltinTypeKind::U8
            | BuiltinTypeKind::U16
            | BuiltinTypeKind::U32
            | BuiltinTypeKind::U64
            | BuiltinTypeKind::U128
            | BuiltinTypeKind::Unsized
            // Shoult this be included? UnsignedBigInt? WHAT?
            | BuiltinTypeKind::BigInt
            | BuiltinTypeKind::Runtime => true,
            _ => false,
        }
    }

    pub fn is_ranged(&self) -> bool {
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
            | BuiltinTypeKind::Str
            | BuiltinTypeKind::Char
            | BuiltinTypeKind::BigInt
            | BuiltinTypeKind::BigFloat
            | BuiltinTypeKind::List
            | BuiltinTypeKind::Set
            | BuiltinTypeKind::Map
            | BuiltinTypeKind::Tuple
            | BuiltinTypeKind::Runtime => true,
            BuiltinTypeKind::Nil | BuiltinTypeKind::Bool => false,
        }
    }

    pub fn is_comparable(&self) -> bool {
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
            | BuiltinTypeKind::Str
            | BuiltinTypeKind::Char
            | BuiltinTypeKind::BigInt
            | BuiltinTypeKind::BigFloat
            | BuiltinTypeKind::Nil
            | BuiltinTypeKind::Bool
            | BuiltinTypeKind::Runtime => true,
            BuiltinTypeKind::List
            | BuiltinTypeKind::Set
            | BuiltinTypeKind::Map
            | BuiltinTypeKind::Tuple => false,
        }
    }

    pub fn is_float(&self) -> bool {
        match self {
            BuiltinTypeKind::F16
            | BuiltinTypeKind::F32
            | BuiltinTypeKind::F64
            | BuiltinTypeKind::F128
            | BuiltinTypeKind::BigFloat
            | BuiltinTypeKind::Runtime => true,
            _ => false,
        }
    }

    pub fn is_character_mappable(&self) -> bool {
        match self {
            BuiltinTypeKind::Str | BuiltinTypeKind::Char | BuiltinTypeKind::Runtime => true,
            _ => false,
        }
    }

    pub fn has_len(&self) -> bool {
        match self {
            BuiltinTypeKind::Str
            | BuiltinTypeKind::Char
            | BuiltinTypeKind::List
            | BuiltinTypeKind::Set
            | BuiltinTypeKind::Map
            | BuiltinTypeKind::Tuple
            | BuiltinTypeKind::Runtime => true,
            _ => false,
        }
    }

    pub fn is_collection(&self) -> bool {
        match self {
            BuiltinTypeKind::Str
            | BuiltinTypeKind::List
            | BuiltinTypeKind::Set
            | BuiltinTypeKind::Map
            | BuiltinTypeKind::Tuple
            | BuiltinTypeKind::Runtime => true,
            _ => false,
        }
    }

    pub fn is_ordered(&self) -> bool {
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
            | BuiltinTypeKind::BigFloat
            | BuiltinTypeKind::Runtime => true,
            _ => false,
        }
    }
}
