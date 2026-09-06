//! Holds external type instantiation helpers
pub mod java_helpers;
pub mod rust_helpers;
use chrn_utils::id_types::InternedId;
use lang::types::externs::ExternPlatformType;

use crate::{
    lookup::scopes::scopes_concepts::ScopeType,
    script_compiler::helpers::instantiation_symbols::{
        InstantiationSymbolBase, InstantiationSymbolKind,
    },
    semantic::hir::hir_symbols::SymbolOrigin,
};

//NOTE: The "entry" naming is because the recursive loop goes off of only scope id and extern kinds,
//which loops just find for all representations, but the entry itself must also follow this ruling.

/// All known namespaces for external types like for "c::unsinged_long_long_int" and "java::int"
pub static ALL_EXTERN_NAMESPACES_DATASET: [&[InstantiationSymbolBase]; 2] = [
    &java_helpers::JAVA_NAMESPACE_ENTRY,
    &rust_helpers::RUST_NAMESPACE_ENTRY,
];

// Expected default for extern types
const EXTERN_SYM_ORIGIN: SymbolOrigin = SymbolOrigin::Compiler;
const EXTERN_SCOPE_TYPE: ScopeType = ScopeType::Compiler;
const EXTERN_IS_PRIV: bool = true;

/// Domain-specific constructor helper for `InstantiationSymbolBase`
const fn new_extern_namespace(
    name_id: u32,
    syms: &'static [InstantiationSymbolBase],
) -> InstantiationSymbolBase {
    InstantiationSymbolBase::new(
        InternedId::new(name_id),
        EXTERN_SYM_ORIGIN,
        EXTERN_SCOPE_TYPE,
        EXTERN_IS_PRIV,
        InstantiationSymbolKind::Namespace(syms),
    )
}

/// Domain-specific constructor helper for `InstantiationSymbolBase`
const fn new_extern_sym(name_id: u32, extern_ty: ExternPlatformType) -> InstantiationSymbolBase {
    InstantiationSymbolBase::new(
        InternedId::new(name_id),
        EXTERN_SYM_ORIGIN,
        EXTERN_SCOPE_TYPE,
        EXTERN_IS_PRIV,
        InstantiationSymbolKind::ExternType(extern_ty),
    )
}

// /// Frame data retention for loading extern kinds iteratively
// pub struct ExternFrame {
//     pub scope_id: ScopeId,
//     pub extern_kinds: &'static [ExternKind],
//     pub pos: usize,
// }
//
// impl ExternFrame {
//     pub fn new(scope_id: ScopeId, extern_kinds: &'static [ExternKind], pos: usize) -> Self {
//         Self {
//             scope_id,
//             extern_kinds,
//             pos,
//         }
//     }
// }
