use common::symbols::SymbolId;

// The value system of script would be simple but the serial does need this too so maybe re-use
// it?
#[derive(Debug, PartialEq)]
pub enum Value {
    Var(SymbolId),
    Integer(i64),
    Float(f64),
    Tuple(Vec<Value>),
    Unknown,
}
