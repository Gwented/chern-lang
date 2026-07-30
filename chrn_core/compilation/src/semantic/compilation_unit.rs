use chrn_utils::id_types::{ImplId, SymbolId};

/// Represents all possible user compilation units
#[derive(Debug, Clone)]
pub enum CompilationUnit {
    Symbol(SymbolId),
    Impl(ImplId),
}
