/// Whatever type inside this container is guaranteed to be valid in whatever the appropriate context
/// is.
// Should this be read-only? What is reading?
use crate::source_map::source_span::SourceSpan;

//TODO: Move me
/// Generic structure for attaching a span to any type
#[derive(Debug, Clone, Eq)]
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

    pub const fn as_opt<'a>(&'a self) -> SpannedOptContainer<'a, T> {
        SpannedOptContainer::new(&self.inner, Some(self.span))
    }
}

impl<T: Eq> PartialEq for SpannedContainer<T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

//WARN: Maybe we shouldn't change the hash to only account for inner?
impl<T: std::hash::Hash> std::hash::Hash for SpannedContainer<T> {
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
    pub const fn new(inner: &'a T, span: SourceSpan) -> Self {
        Self { inner, span }
    }
}

impl<T: Clone> SpannedContainerRef<'_, T> {
    // Should this transfer ownership?
    /// Converts borrowed `self` into owned `SpannedContainer`
    pub fn into_owned(&self) -> SpannedContainer<T> {
        SpannedContainer::new(self.inner.clone(), self.span)
    }
}

/// Generic structure for attaching a span to any type, which may or may not have a span
#[derive(Debug, Clone)]
pub struct SpannedOptContainer<'a, T> {
    pub inner: &'a T,
    pub span: Option<SourceSpan>,
}

impl<'a, T> SpannedOptContainer<'a, T> {
    pub const fn new(inner: &'a T, span: Option<SourceSpan>) -> Self {
        Self { inner, span }
    }
}

impl<'a, T: std::hash::Hash> std::hash::Hash for SpannedOptContainer<'a, T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}

pub struct CheckedContainer<T> {
    pub inner: T,
}

impl<T> CheckedContainer<T> {
    pub fn new(inner: T) -> Self {
        Self { inner }
    }
    pub fn inner(&self) -> &T {
        &self.inner
    }
    pub fn as_ref(&self) -> CheckedContainerRef<'_, T> {
        CheckedContainerRef::new(&self.inner)
    }
}

pub struct CheckedContainerRef<'a, T> {
    pub inner: &'a T,
}

impl<'a, T> CheckedContainerRef<'a, T> {
    pub fn new(inner: &'a T) -> Self {
        Self { inner }
    }
}
