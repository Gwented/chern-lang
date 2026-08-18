//! Chrn options that can be selected externally for extra chrn compiler behavior.
//!
//! The main intention of the config is to act as an abstraction layer that only works if it was
//! actually selected. This reduces the need for any caller to care about how it's called, the
//! internals entirely handle whether or not anything is actually used.
pub mod chrn_logger;
pub mod chrn_perf;

use crate::chrn_config::{chrn_logger::ChrnConfigLogger, chrn_perf::ChrnPerfTracker};

//TEST: No longer has use but is useful to keep in case of any future use
/// Config given before running a chrn language instance, which allows for external tooling
/// capabilities, such as cli args.
#[derive(Debug, Default)]
pub struct ChrnConfig {
    // This is purposefully nested so that it owns the specific methods for logging as to not convolute
    // `ChrnConfig`
    /// `struct` that contains a single boolean which determines whether or not debug logging will
    /// be done.
    logger: ChrnConfigLogger,
    perf_tracker: ChrnPerfTracker,
}

impl ChrnConfig {
    pub const fn new(logger: ChrnConfigLogger, perf_tracker: ChrnPerfTracker) -> ChrnConfig {
        ChrnConfig {
            logger,
            perf_tracker,
        }
    }

    pub const fn logger(&self) -> &ChrnConfigLogger {
        &self.logger
    }

    pub const fn perf_tracker(&self) -> &ChrnPerfTracker {
        &self.perf_tracker
    }

    pub const fn builder() -> ChrnConfigBuilder {
        ChrnConfigBuilder {
            logger: None,
            perf_tracker: None,
        }
    }
}

/// Builder for `ChrnConfig`
pub struct ChrnConfigBuilder {
    logger: Option<ChrnConfigLogger>,
    perf_tracker: Option<ChrnPerfTracker>,
}

impl ChrnConfigBuilder {
    pub const fn build(self) -> ChrnConfig {
        let logger = if let Some(inner) = self.logger {
            inner
        } else {
            ChrnConfigLogger::new(false)
        };

        let perf_tracker = if let Some(perf) = self.perf_tracker {
            perf
        } else {
            ChrnPerfTracker::new(None)
        };

        ChrnConfig {
            logger,
            perf_tracker: perf_tracker,
        }
    }

    pub const fn add_logger(mut self) -> Self {
        self.logger = Some(ChrnConfigLogger::new(true));
        self
    }
}
