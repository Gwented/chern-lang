use std::fmt::Debug;

use crate::source_map::source_span::SourceSpan;

//TEST: May or may not be used
/// Generic structure for attaching a span to any type
#[derive(Debug, Clone)]
pub struct SpannedContainer<T> {
    pub inner: T,
    pub span: SourceSpan,
}

impl<T> SpannedContainer<T> {
    pub fn new(inner: T, span: SourceSpan) -> SpannedContainer<T> {
        SpannedContainer { inner, span }
    }
}

/// Type-safe wrapper for using an index that contains a valid index into `Intern`
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InternedId {
    pub id: u32,
}

impl InternedId {
    pub fn new(id: u32) -> InternedId {
        InternedId { id }
    }
}

/// Type-safe wrapper for using an index that contains a valid index into a source region
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SourceRegionId {
    pub id: u32,
}

impl SourceRegionId {
    pub fn new(id: u32) -> SourceRegionId {
        SourceRegionId { id }
    }
}

// Not using a span arena unless smoe sort of bottleneck is reached somehow
// #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
// pub struct SpanId {
//     pub id: u32,
// }
//
// impl SpanId {
//     pub fn new(id: u32) -> SpanId {
//         SpanId { id }
//     }
// }

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PathId {
    pub id: u32,
}

impl PathId {
    pub fn new(id: u32) -> PathId {
        PathId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TypeId {
    pub id: u32,
}

impl TypeId {
    pub fn new(id: u32) -> TypeId {
        TypeId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ConfigId {
    pub id: u32,
}

impl ConfigId {
    pub fn new(id: u32) -> ConfigId {
        ConfigId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId {
    pub id: u32,
}

impl SymbolId {
    pub fn new(id: u32) -> SymbolId {
        SymbolId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModuleId {
    pub id: usize,
}

impl ModuleId {
    pub fn new(id: usize) -> ModuleId {
        ModuleId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ScopeId {
    pub id: usize,
}

impl ScopeId {
    pub fn new(id: usize) -> ScopeId {
        ScopeId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AstId {
    pub id: u32,
}

impl AstId {
    pub fn new(id: u32) -> AstId {
        AstId { id }
    }
}

/// Type-safe wrapper for using an index that contains a valid index into a symbol of some kind
/// that would be considered a member
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct MemberId {
    pub id: u32,
}

impl MemberId {
    pub fn new(id: u32) -> MemberId {
        MemberId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ExprId {
    pub id: u32,
}

impl ExprId {
    pub fn new(id: u32) -> ExprId {
        ExprId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ValueId {
    pub id: u32,
}

impl ValueId {
    pub fn new(id: u32) -> ValueId {
        ValueId { id }
    }
}
