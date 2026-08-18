use std::cell::Cell;

// -- RECURSION --
/// General purpose structure that creates guards when increasing in depth, that drop when the call
/// stack is popped.
///
/// When the limit is reached returns a basic Err(())
#[derive(Debug)]
pub struct RecursionTracker {
    depth: Cell<u16>,
    limit: u16,
}

impl RecursionTracker {
    pub fn new(limit: u16) -> RecursionTracker {
        RecursionTracker {
            depth: Cell::new(0),
            limit,
        }
    }

    pub fn depth(&self) -> u16 {
        self.depth.get()
    }

    pub fn limit(&self) -> u16 {
        self.limit
    }

    /// Creates recursive-depth tracking guard
    ///
    /// Returns `Ok` guard if the limit has not been reached.
    /// Returns `Err` if an extra guard would exceed `self.limit`
    pub fn increase_depth(&self) -> Result<RecursiveGuard<'_>, ()> {
        if self.limit <= self.depth.get() {
            return Err(());
        }

        self.increment_depth(1);

        Ok(RecursiveGuard { tracker: self })
    }

    fn increment_depth(&self, amt: u16) {
        self.depth.set(self.depth.get() + amt);
    }
}

#[derive(Debug)]
pub struct RecursiveGuard<'a> {
    tracker: &'a RecursionTracker,
}

impl Drop for RecursiveGuard<'_> {
    fn drop(&mut self) {
        self.tracker.depth.set(self.tracker.depth.get() - 1);
    }
}
