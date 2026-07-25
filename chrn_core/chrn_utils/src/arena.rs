use std::{
    marker::PhantomData,
    ops::{Index, IndexMut},
};

use crate::id_types::ArenaIndex;

/// Generic `Arena` which holds `items` of type `T` and an index of `I`.
///
/// This is to reduce the duplication of basic arena types that just want to enforce type-safe
/// indexing operations.
#[derive(Debug, Default)]
pub struct Arena<T, I: ArenaIndex> {
    pub items: Vec<T>,
    /// Stored type to index with using `I`
    _marker: PhantomData<I>,
}

impl<T, I: ArenaIndex> Arena<T, I> {
    pub const fn new() -> Arena<T, I> {
        Arena {
            items: Vec::new(),
            _marker: PhantomData,
        }
    }

    pub fn with_capacity(capacity: usize) -> Arena<T, I> {
        Arena {
            items: Vec::with_capacity(capacity),
            _marker: PhantomData,
        }
    }

    /// Pushes `T` then returns the asociated index `I`.
    pub fn push(&mut self, val: T) -> I {
        let idx = self.items.len();
        self.items.push(val);
        I::from_usize(idx)
    }

    /// Gets `Option<T>` by using index `I`.
    pub fn get(&self, idx: I) -> Option<&T> {
        self.items.get(idx.into_usize())
    }

    /// Gets `Option<T>` by using index `I`.
    pub fn get_mut(&mut self, idx: I) -> Option<&mut T> {
        self.items.get_mut(idx.into_usize())
    }

    /// Removes `T` using `I` then returns `T`, panics upon index out of bounds.
    pub fn remove(&mut self, idx: I) -> T {
        self.items.remove(idx.into_usize())
    }

    /// Removes `T` using `I` using O(1) swap remove then returns `T`, panics upon index out of bounds.
    pub fn swap_remove(&mut self, idx: I) -> T {
        self.items.swap_remove(idx.into_usize())
    }

    /// Wrapper for `len()` call for internal `items`
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Wrapper for `len()` call for internal `items`
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Iterates over items, returning references
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.items.iter()
    }

    /// Iterates over items, returning mutable references
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.items.iter_mut()
    }
}

impl<T, I: ArenaIndex> IntoIterator for Arena<T, I> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a, T, I: ArenaIndex> IntoIterator for &'a Arena<T, I> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

impl<'a, T, I: ArenaIndex> IntoIterator for &'a mut Arena<T, I> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter_mut()
    }
}

impl<T, I: ArenaIndex> Index<I> for Arena<T, I> {
    type Output = T;

    fn index(&self, idx: I) -> &Self::Output {
        &self.items[idx.into_usize()]
    }
}

impl<T, I: ArenaIndex> IndexMut<I> for Arena<T, I> {
    fn index_mut(&mut self, idx: I) -> &mut Self::Output {
        &mut self.items[idx.into_usize()]
    }
}

impl<T, I: ArenaIndex> From<Vec<T>> for Arena<T, I> {
    fn from(vec: Vec<T>) -> Self {
        Arena {
            items: vec,
            _marker: PhantomData,
        }
    }
}
