use common::fmter::{Formattable, Formatted};

use crate::id_types::{NameId, ValueId};

// The value system of script would be simple but the serial does need this too so maybe re-use
// it?
#[derive(Debug)]
pub enum Value {
    // For > i128
    // BigInt(NameId),
    I128(i128),
    F64(f64),
    Bool(bool),
    Char(char),
    Tuple(Vec<Value>),
    CompileStr(NameId),
    RuntimeStr(String),
    Unknown,
}

impl Value {
    pub fn kind(&self) -> ValueKind {
        match self {
            Value::I128(_) => ValueKind::I128,
            Value::F64(_) => ValueKind::F64,
            Value::Bool(_) => ValueKind::Bool,
            Value::Char(_) => ValueKind::Char,
            Value::Tuple(_) => ValueKind::Tuple,
            Value::CompileStr(_) => ValueKind::InternedStr,
            Value::RuntimeStr(_) => ValueKind::RuntimeStr,
            Value::Unknown => ValueKind::Unknown,
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
    I128,
    F64,
    Char,
    Bool,
    Tuple,
    InternedStr,
    RuntimeStr,
    Unknown,
}

impl Formattable for ValueKind {
    fn to_fmt(&self) -> common::fmter::Formatted {
        match self {
            ValueKind::I128 => Formatted::I128,
            ValueKind::F64 => Formatted::F64,
            ValueKind::Char => Formatted::Char,
            ValueKind::Tuple => Formatted::Tuple,
            ValueKind::Bool => Formatted::Bool,
            ValueKind::InternedStr | ValueKind::RuntimeStr => Formatted::Str,
            ValueKind::Unknown => Formatted::Nothing,
        }
    }
}

pub enum ValueResult {
    Resolved(ValueId),
    Unresolved,
}
