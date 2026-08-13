//NOTE: Name may conflict when the distinction between resolvers and checkers are better reflected
//inside `resolvers/`

use std::{collections::HashSet, hash::Hash};

use chrn_utils::id_types::{InternedId, SpannedContainer};

// ????
/// Hashes and tracks types generically
#[derive(Debug)]
pub(crate) struct DuplicateTracker<T: Clone + Hash + Eq> {
    pub(crate) seen: HashSet<T>,
    pub(crate) found_dups: Vec<DuplicateFound<T>>,
}

impl<T: Clone + Hash + Eq> DuplicateTracker<T> {
    pub(crate) fn new() -> Self {
        Self {
            seen: HashSet::new(),
            found_dups: Vec::new(),
        }
    }

    pub(crate) fn with_capacities(hash_cap: usize, found_dups_cap: usize) -> Self {
        Self {
            seen: HashSet::with_capacity(hash_cap),
            found_dups: Vec::with_capacity(found_dups_cap),
        }
    }

    // ???
    /// If the given element already existed, it is stored inside `found_dups`. Returns `true`.
    /// If the given element didn't exist, it's normally inserted. Returns `false`.
    pub(crate) fn insert_or_store(&mut self, given: T) -> bool {
        if self.contains(&given) {
            let original = self.seen.get(&given).expect("Just checked");
            let duplicate = DuplicateFound::new(original.clone(), given);
            self.found_dups.push(duplicate);
            return true;
        }
        // DROPPED? Ok.
        self.seen.insert(given);
        false
    }

    /// Checks if `self.seen` contains `given`
    pub(crate) fn contains(&self, given: &T) -> bool {
        self.seen.contains(given)
    }

    // LOAD BEARING
    /// Clears all of inner for another round of tracking
    pub(crate) fn clear(&mut self) {
        self.seen.clear();
        self.found_dups.clear();
    }
}

/// ?
#[derive(Debug)]
pub(crate) struct DuplicateFound<T> {
    /// Identifier that had the identifier first
    pub(crate) original: T,
    /// Identifier that copied the original identifier
    pub(crate) dup: T,
}

impl<T: Hash> DuplicateFound<T> {
    pub(crate) fn new(original: T, dup: T) -> Self {
        Self { original, dup }
    }
}

//NOTE: This was here before the above structures and may be removed
//
pub(crate) enum DuplicateIdentResult {
    NoDuplicate,
    Duplicate {
        sp_original: SpannedContainer<InternedId>,
        sp_dup: SpannedContainer<InternedId>,
    },
}

// Ok but what if this was const?
/// Checks if `src_idents`
pub fn check_duplicate_ident(idents: &[SpannedContainer<InternedId>]) -> DuplicateIdentResult {
    for (i, current_ident) in idents.iter().enumerate() {
        if let Some((_, original)) = idents
            .iter()
            .enumerate()
            // If the other index was declared after the current index and they have the same identifier
            //
            // Since this iteration specifically checks if the current was declared after the
            // last and the iteration terminates upon the first match, this correctly points at
            // the original field for all duplicates.
            .find(|(other_i, f)| *other_i < i && current_ident.inner == f.inner)
        {
            return DuplicateIdentResult::Duplicate {
                sp_original: original.clone(),
                sp_dup: current_ident.clone(),
            };
        }
    }
    DuplicateIdentResult::NoDuplicate
}
