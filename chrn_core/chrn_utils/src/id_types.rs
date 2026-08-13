use std::{fmt::Debug, hash::Hash};

use crate::source_map::source_span::SourceSpan;

//TODO: Move me
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

impl<T: Eq> Eq for SpannedContainer<T> {}
impl<T: Eq> PartialEq for SpannedContainer<T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<T: Hash> Hash for SpannedContainer<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
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
    ImplMemberId,
    ExternTypeId,
);
arena_idx_impl_u16!(ScopeId);

macro_rules! id_type_impl_u32 {
    ($($ident:ident),* $(,)?) => {
        $(
        /// Type-safe wrapper for using an index that contains a valid id
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $ident {
            pub id: u32
        }

        impl $ident {
            pub const fn new(id: u32) -> Self {
                Self { id }
            }
        }
        )*
    }
}

macro_rules! id_type_impl_u16 {
    ($($ident:ident),* $(,)?) => {
        $(
        /// Type-safe wrapper for using an index that contains a valid id
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $ident {
            pub id: u16
        }

        impl $ident {
            pub const fn new(id: u16) -> Self {
                Self { id }
            }
        }
        )*
    }
}

id_type_impl_u32!(
    InternedId,
    SourceRegionId,
    PathId,
    TypeId,
    ExternTypeId,
    VariableId,
    ConfigRootId,
    SymbolId,
    DirectiveId,
    ModuleId,
    AstId,
    MemberId,
    ExprId,
    ValueId,
    ImplId,
    ImplMemberId,
);
id_type_impl_u16!(ScopeId);
