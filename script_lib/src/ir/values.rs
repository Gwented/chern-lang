use common::symbols::SymbolId;

use crate::semantic::representation::{SymbolInfo, TypeInfo};

// The value system of script would be simple but the serial does need this too so maybe re-use
// it?
#[derive(Debug)]
pub enum Value {
    Var(SymbolInfo),
    I128(i128),
    U128(u128),
    F64(f64),
    // I64(i64),
    Char(char),
    Tuple(Vec<Value>),
    Str(TypeInfo),
    Unknown,
}
