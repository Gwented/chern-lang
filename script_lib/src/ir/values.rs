use common::symbols::SymbolId;

use crate::semantic::representation::{SymbolInfo, TypeInfo};

// The value system of script would be simple but the serial does need this too so maybe re-use
// it?
#[derive(Debug)]
pub enum Value {
    Var(SymbolInfo),
    I64(i64),
    F64(f64),
    Char(char),
    Tuple(Vec<Value>),
    Str(TypeInfo),
    Unknown,
}
