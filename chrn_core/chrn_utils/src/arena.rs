//TEST: May make structures like InternedId into _marker: I as well as for arenas
use std::{
    marker::PhantomData,
    ops::{Index, IndexMut},
};

use crate::id_types::{ArenaIndex, InternedId};

// macro_rules! from_usize_impl {
//     ($($t:ty),* $(,)?) => {
//         $(
//             impl Into<usize> for $t {
//                 fn into(self) -> usize {
//                     self 
//                 }
//             }
//         )*
//     }
// }
//
// from_usize_impl!(InternedId);

// TEST:
/// Generic `Arena` which holds `items` of type `T` and an index of `I`.
///
/// This is to reduce the duplication of basic arena types that just want to enforce type-safe
/// indexing operations.
#[derive(Debug)]
pub struct Arena<T, I: ArenaIndex> {
    pub items: Vec<T>,
    /// Stored type to index with using `I`
    _marker: PhantomData<I>,
}

impl<T, I: ArenaIndex> Arena<T, I> {
    pub fn new() -> Arena<T, I> {
        Arena {
            items: Vec::new(),
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

    /// Iterates over items, returning references
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.items.iter()
    }

    /// Iterates over items, returning mutable references
    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
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

// impl<T, I: ArenaIndex> Iterator for Arena<T, I> {}
