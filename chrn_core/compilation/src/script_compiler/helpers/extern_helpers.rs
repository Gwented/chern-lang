//! Intended to hold external type instantiation helpers
pub mod java_helpers;
use chrn_utils::id_types::{InternedId, ScopeId};

/// All known namespaces for external types like for "c::unsinged_long_long_int" and "java::int"
pub static ALL_EXTERN_NAMESPACES: [&[ExternKind]; 1] = [&java_helpers::JAVA_NAMESPACE];

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

// struct ExternSymbol {
//     pub(super) name_id: InternedId,
// }

// impl ExternSymbol {
//     const fn new(name_id: InternedId) -> Self {
//         Self { name_id }
//     }
// }
