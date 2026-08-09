// The value system of script would be simple but the serial does need this too so maybe re-use
// it?

use chrn_utils::id_types::{ExprId, InternedId, SymbolId, TypeId};

use crate::{
    fmter::{Formattable, Formatted},
    types::boundaries::TypeBoundaryFlags,
};
// TODO: Should probably be in compilation
// Was about to say this again..
// And again

// This is supposed to represent something like, let x = 4, where 4 may or may not have a constant
// value, 4 is the expression, and it's type is whatever is inferred
#[derive(Debug, Clone)]
pub struct ValueInfo {
    pub type_id: TypeId,
    pub expr_id: ExprId,
    pub const_val: Option<Value>,
}

impl ValueInfo {
    pub fn new(type_id: TypeId, expr_id: ExprId, const_val: Option<Value>) -> ValueInfo {
        ValueInfo {
            type_id,
            expr_id,
            const_val,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    // For > i128
    // BigInt(NameId),
    I64(i64),
    F64(f64),
    Bool(bool),
    Char(char),
    Func(SymbolId),
    Tuple(Vec<Value>),
    Array(Vec<Value>),
    InternedStr(InternedId),
    RuntimeStr(String),
    Unknown,
}

impl Value {
    pub fn kind(&self) -> ValueKind {
        match self {
            Value::I64(_) => ValueKind::I64,
            Value::F64(_) => ValueKind::F64,
            Value::Bool(_) => ValueKind::Bool,
            Value::Char(_) => ValueKind::Char,
            Value::Tuple(_) => ValueKind::Tuple,
            Value::InternedStr(_) => ValueKind::InternedStr,
            Value::RuntimeStr(_) => ValueKind::RuntimeStr,
            Value::Func(_) => ValueKind::Func,
            Value::Unknown => ValueKind::Unknown,
            Value::Array(_) => ValueKind::Array,
        }
    }

    pub fn is_bool(&self) -> bool {
        match self {
            Value::Bool(_) => true,
            _ => false,
        }
    }
    // No
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ValueKind {
    // BigInt,
    I64,
    F64,
    Char,
    Func,
    Bool,
    Tuple,
    InternedStr,
    RuntimeStr,
    Array,
    Unknown,
}

impl ValueKind {
    /// Returns valid boundary on `Some`
    /// On `None`, it means that the value has no actual boundary associated with it.
    ///
    /// NOTE: Since language concepts like tuples and arrays themselves actually have boundaries,
    /// those will be given, no recursive resolution related to the value is done so that should be
    /// handled accordingly externally.
    pub fn boundaries(self) -> Option<TypeBoundaryFlags> {
        match self {
            ValueKind::I64 => Some(TypeBoundaryFlags::SIGNED_INTEGER),
            ValueKind::F64 => Some(TypeBoundaryFlags::FLOAT),
            ValueKind::Char => Some(TypeBoundaryFlags::CHAR),
            // Not sure if values themselves are a good interface for getting the boundary of a flag
            // Maybe this should stay an Option
            ValueKind::Bool => Some(TypeBoundaryFlags::BOOL),
            ValueKind::InternedStr | ValueKind::RuntimeStr => Some(TypeBoundaryFlags::STR),
            ValueKind::Array | ValueKind::Tuple => Some(TypeBoundaryFlags::COLLECTION),
            // Should an `Unknown` boundary exist? That seems like it would complicate things
            ValueKind::Func | ValueKind::Unknown => None,
        }
    }
}

impl Formattable for ValueKind {
    fn to_fmt(&self) -> Formatted {
        match self {
            ValueKind::I64 => Formatted::I64,
            ValueKind::F64 => Formatted::F64,
            ValueKind::Char => Formatted::Char,
            ValueKind::Tuple => Formatted::Tuple,
            ValueKind::Bool => Formatted::Bool,
            ValueKind::InternedStr | ValueKind::RuntimeStr => Formatted::Str,
            ValueKind::Unknown => Formatted::Unknown,
            ValueKind::Array => Formatted::Array,
            ValueKind::Func => Formatted::Func,
        }
    }
}
