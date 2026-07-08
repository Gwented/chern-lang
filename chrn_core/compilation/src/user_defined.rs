use chrn_utils::id_types::{AstId, ModuleId, SymbolId, TypeId};

use crate::semantic::hir::hir_concepts::SymbolKind;

// This name sounds a little misleading
// TEST:
/// General structure that holds metadata that proves a symbol is user defined.
#[derive(Clone, Copy)]
pub struct UserDefinedMetadata {
    pub sym_id: SymbolId,
    pub type_id: TypeId,
    pub mod_id: ModuleId,
    pub ast_id: AstId,
    pub kind: SymbolKind,
}

impl UserDefinedMetadata {
    pub fn new(
        sym_id: SymbolId,
        type_id: TypeId,
        ast_id: AstId,
        mod_id: ModuleId,
        kind: SymbolKind,
    ) -> UserDefinedMetadata {
        UserDefinedMetadata {
            sym_id,
            type_id,
            mod_id,
            ast_id,
            kind,
        }
    }
}
