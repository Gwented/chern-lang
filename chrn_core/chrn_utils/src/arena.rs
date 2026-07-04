//TEST: May make structures like InternedId into _marker: I as well as for arenas
use std::marker::PhantomData;

use crate::id_types::InternedId;

// macro_rules! from_usize_impl {
//     ($($t:ty),* $(,)?) => {
//         $(
//             impl Into<usize> for $t {
//                 fn into(self) -> usize {
//                     self.id as usize
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
pub struct Arena<T, I: Into<usize> + From<usize>> {
    pub items: Vec<T>,
    _marker: PhantomData<I>,
}

impl<T, I: Into<usize> + From<usize>> Arena<T, I> {
    /// Pushes `T` then returns the asociated index `I`
    fn push(&mut self, val: T) -> I {
        let idx = self.items.len();
        self.items.push(val);
        I::from(idx)
    }

    /// Gets `Option<T>` by using index `I`
    fn get(&self, idx: I) -> Option<&T> {
        self.items.get(idx.into())
    }

    /// Gets `Option<T>` by using index `I`
    fn get_mut(&mut self, idx: I) -> Option<&mut T> {
        self.items.get_mut(idx.into())
    }
}
