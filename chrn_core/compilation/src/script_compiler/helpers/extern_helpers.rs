//! Holds external type instantiation helpers
pub mod java_helpers;
use chrn_utils::id_types::{InternedId, ScopeId};

use crate::{
    lookup::scopes::scopes_concepts::ScopeType,
    script_compiler::helpers::common_helpers::{
        CommonKind, CommonNamespaceParts, CommonSymbolBase, CommonSymbolKind, CommonSymbolParts,
    },
    semantic::hir::hir_symbols::{SymbolKind, SymbolOrigin},
};
// Explain more buddy

//NOTE: The "entry" naming is because the recursive loop goes off of only scope id and extern kinds,
//which loops just find for all representations, but the entry itself must also follow this ruling.

/// All known namespaces for external types like for "c::unsinged_long_long_int" and "java::int"
// pub static ALL_EXTERN_NAMESPACES_DATASET: [&[ExternKind]; 1] =
//     [&java_helpers::JAVA_NAMESPACE_ENTRY];

pub static ALL_EXTERN_NAMESPACES_DATASET: [&[CommonSymbolBase]; 1] =
    [&java_helpers::JAVA_NAMESPACE_ENTRY];

// Expected default for extern types
const EXTERN_SYM_ORIGIN: SymbolOrigin = SymbolOrigin::Compiler;
const EXTERN_SCOPE_TYPE: ScopeType = ScopeType::Compiler;
const EXTERN_IS_PRIV: bool = true;

// Domain-specific constructors
/// Constructor helper for `CommonSymbolNamespace`
const fn new_extern_namespace(name_id: u32, syms: &'static [CommonSymbolBase]) -> CommonSymbolBase {
    CommonSymbolBase::new(
        InternedId::new(name_id),
        EXTERN_SYM_ORIGIN,
        EXTERN_SCOPE_TYPE,
        EXTERN_IS_PRIV,
        CommonSymbolKind::Namespace(syms),
    )
    // CommonNamespaceParts::new(
    //     InternedId::new(name_id),
    //     EXTERN_SYM_ORIGIN,
    //     EXTERN_SCOPE_TYPE,
    //     true,
    //     syms,
    // )
}

/// Constructor helper for `CommonSymbolParts`
const fn new_extern_sym(name_id: u32) -> CommonSymbolBase {
    CommonSymbolBase::new(
        InternedId::new(name_id),
        EXTERN_SYM_ORIGIN,
        EXTERN_SCOPE_TYPE,
        EXTERN_IS_PRIV,
        CommonSymbolKind::ExternType,
    )
    // CommonSymbolParts::new(
    //     InternedId::new(name_id),
    //     SymbolKind::ExternType,
    //     EXTERN_SYM_ORIGIN,
    //     EXTERN_SCOPE_TYPE,
    //     true,
    // )
}

// -- Things --
pub enum ExternKind {
    Namespace(ExternNamespace),
    Symbol(InternedId),
}

pub struct ExternNamespace {
    pub name_id: InternedId,
    pub syms: &'static [ExternKind],
}

impl ExternNamespace {
    const fn new(name_id: InternedId, syms: &'static [ExternKind]) -> Self {
        Self { name_id, syms }
    }
}

pub struct ExternSymbol {
    pub(super) name_id: InternedId,
}

impl ExternSymbol {
    const fn new(name_id: InternedId) -> Self {
        Self { name_id }
    }
}

// IGNORE
/// Frame data retention for loading extern kinds iteratively
pub struct ExternFrame {
    pub scope_id: ScopeId,
    pub extern_kinds: &'static [ExternKind],
    pub pos: usize,
}

impl ExternFrame {
    pub fn new(scope_id: ScopeId, extern_kinds: &'static [ExternKind], pos: usize) -> Self {
        Self {
            scope_id,
            extern_kinds,
            pos,
        }
    }
}
