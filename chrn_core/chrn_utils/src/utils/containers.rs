/// Whatever type inside this container is guaranteed to be valid in whatever the appropriate context
/// is.
// Should this be read-only? What is reading?
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
