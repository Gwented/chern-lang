use chrn_utils::id_types::{SymbolId, TypeId};

pub enum ResolvedPath {
    Symbol(SymbolId),
    Type(TypeId),
}
