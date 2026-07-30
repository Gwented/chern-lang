use std::fmt::Debug;

use crate::source_map::source_span::SourceSpan;

/// Generic structure for attaching a span to any type
#[derive(Debug, Clone)]
pub struct SpannedContainer<T> {
    pub inner: T,
    pub span: SourceSpan,
}

impl<T> SpannedContainer<T> {
    pub const fn new(inner: T, span: SourceSpan) -> SpannedContainer<T> {
        SpannedContainer { inner, span }
    }
    pub const fn as_ref<'a>(&'a self) -> SpannedContainerRef<'a, T> {
        SpannedContainerRef::new(&self.inner, self.span)
    }
}

/// Generic structure for attaching a span to any type reference
#[derive(Debug, Clone)]
pub struct SpannedContainerRef<'a, T> {
    pub inner: &'a T,
    pub span: SourceSpan,
}

impl<'a, T> SpannedContainerRef<'a, T> {
    pub const fn new(inner: &'a T, span: SourceSpan) -> SpannedContainerRef<'a, T> {
        SpannedContainerRef { inner, span }
    }
}

impl<T: Clone> SpannedContainerRef<'_, T> {
    // Should this transfer ownership?
    /// Converts borrowed `self` into owned `SpannedContainer`
    pub fn into_owned(&self) -> SpannedContainer<T> {
        SpannedContainer::new(self.inner.clone(), self.span)
    }
}

pub trait ArenaIndex: Copy {
    fn into_usize(self) -> usize;
    fn from_usize(val: usize) -> Self;
}

/// Convenience for `u32` containing implementers of `ArenaIndex`
macro_rules! arena_idx_impl_u32 {
    ($($t:ty),* $(,)?) => {
        $(
            impl ArenaIndex for $t {
                fn into_usize(self) -> usize {
                    self.id as usize
                }

                fn from_usize(val: usize) -> Self {
                    Self{id: val as u32}
                }
            }
        )*
    }
}

/// Convenience for `u16` containing implementers of `ArenaIndex`
macro_rules! arena_idx_impl_u16 {
    ($($t:ty),* $(,)?) => {
        $(
            impl ArenaIndex for $t {
                fn into_usize(self) -> usize {
                    self.id as usize
                }

                fn from_usize(val:usize) -> Self {
                    Self{id:val as u16}
                }
            }
        )*
    }
}

arena_idx_impl_u32!(
    InternedId,
    SourceRegionId,
    TypeId,
    VariableId,
    ConfigRootId,
    DirectiveId,
    AstId,
    MemberId,
    SymbolId,
    ExprId,
    ModuleId,
    ValueId,
    ImplId,
    ImplMemberId
);
arena_idx_impl_u16!(ScopeId);

// All id types
/// Type-safe wrapper for using an index that contains a valid index into `Intern`
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InternedId {
    pub id: u32,
}

impl InternedId {
    pub const fn new(id: u32) -> InternedId {
        InternedId { id }
    }
}

/// Type-safe wrapper for using an index that contains a valid index into a source region
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SourceRegionId {
    pub id: u32,
}

impl SourceRegionId {
    pub const fn new(id: u32) -> SourceRegionId {
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
//     pub const fn new(id: u32) -> SpanId {
//         SpanId { id }
//     }
// }

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PathId {
    pub id: u32,
}

impl PathId {
    pub const fn new(id: u32) -> PathId {
        PathId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TypeId {
    pub id: u32,
}

impl TypeId {
    pub const fn new(id: u32) -> TypeId {
        TypeId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct VariableId {
    pub id: u32,
}

impl VariableId {
    pub const fn new(id: u32) -> VariableId {
        VariableId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ConfigRootId {
    pub id: u32,
}

impl ConfigRootId {
    pub const fn new(id: u32) -> ConfigRootId {
        ConfigRootId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId {
    pub id: u32,
}

impl SymbolId {
    pub const fn new(id: u32) -> SymbolId {
        SymbolId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct DirectiveId {
    pub id: u32,
}

impl DirectiveId {
    pub const fn new(id: u32) -> DirectiveId {
        DirectiveId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModuleId {
    pub id: u32,
}

impl ModuleId {
    pub const fn new(id: u32) -> ModuleId {
        ModuleId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ScopeId {
    pub id: u16,
}

impl ScopeId {
    pub const fn new(id: u16) -> ScopeId {
        ScopeId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AstId {
    pub id: u32,
}

impl AstId {
    pub const fn new(id: u32) -> AstId {
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
    pub const fn new(id: u32) -> MemberId {
        MemberId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ExprId {
    pub id: u32,
}

impl ExprId {
    pub const fn new(id: u32) -> ExprId {
        ExprId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ValueId {
    pub id: u32,
}

impl ValueId {
    pub const fn new(id: u32) -> ValueId {
        ValueId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ImplId {
    pub id: u32,
}

impl ImplId {
    pub const fn new(id: u32) -> ImplId {
        ImplId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ImplMemberId {
    pub id: u32,
}

impl ImplMemberId {
    pub const fn new(id: u32) -> Self {
        Self { id }
    }
}
