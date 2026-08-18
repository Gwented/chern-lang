//! General purpose performance tracker

use std::time::Duration;

#[derive(Debug)]
pub struct PerfTracker {
    time_spent: Duration,
}
