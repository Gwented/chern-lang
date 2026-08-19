//! General purpose performance tracker

use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy)]
pub struct PerfTracker {
    start: Instant,
}

impl PerfTracker {
    pub fn new(start: Instant) -> Self {
        Self { start }
    }

    pub fn stop(self) -> PerfOutput {
        PerfOutput::new(self.start.elapsed(), 1)
    }
}

/// Output of information collected from `PerfTracker`
#[derive(Debug, Clone, Copy)]
pub struct PerfOutput {
    pub elapsed: Duration,
    /// Amount of times this duration has been added to
    pub times: u16,
}

impl PerfOutput {
    pub const fn new(elapsed: Duration, times: u16) -> Self {
        Self { elapsed, times }
    }

    // Does this make sense??
    pub fn merge(&mut self, other: PerfOutput) {
        self.elapsed += other.elapsed;
        self.times += other.times;
    }
}
