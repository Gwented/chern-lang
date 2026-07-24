use std::fmt::Display;

//TEST: No longer has use but is useful to keep in case of any future use
/// Config given before running a chrn language instance, which allows for external tooling
/// capabilities, such as cli args.
#[derive(Debug, Default)]
pub struct ChrnConfig {
    // This is purposefully nested so that it owns the specific methods for logging as to not convolute
    // `ChrnConfig`
    logger: ChrnConfigLogger,
}

impl ChrnConfig {
    pub const fn new(logger: ChrnConfigLogger) -> ChrnConfig {
        ChrnConfig { logger }
    }

    pub const fn logger(&self) -> &ChrnConfigLogger {
        &self.logger
    }

    pub const fn builder() -> ChrnConfigBuilder {
        ChrnConfigBuilder { logger: None }
    }
}

//TEST: Can't be const..
impl From<ChrnConfigBuilder> for ChrnConfig {
    fn from(builder: ChrnConfigBuilder) -> Self {
        let logger = if let Some(inner) = builder.logger {
            inner
        } else {
            ChrnConfigLogger::new(false)
        };

        ChrnConfig { logger }
    }
}

//TEST:
pub struct ChrnConfigBuilder {
    logger: Option<ChrnConfigLogger>,
}

impl ChrnConfigBuilder {
    pub const fn build(self) -> ChrnConfig {
        let logger = if let Some(inner) = self.logger {
            inner
        } else {
            ChrnConfigLogger::new(false)
        };

        ChrnConfig { logger }
    }

    pub const fn add_logger(mut self) -> Self {
        self.logger = Some(ChrnConfigLogger::new(true));
        self
    }
}

#[derive(Debug, Default)]
pub struct ChrnConfigLogger {
    can_log: bool,
}

impl ChrnConfigLogger {
    pub const fn new(can_log: bool) -> ChrnConfigLogger {
        ChrnConfigLogger { can_log }
    }

    /// Prints msg with [DBG] header
    pub fn log_dbg<F, T>(&self, f: F)
    where
        F: FnOnce() -> T,
        T: Display,
    {
        if self.can_log {
            println!("[DBG] {}", f())
        }
    }

    /// Prints msg with [WRN] header
    pub fn log_warn<F, T>(&self, f: F)
    where
        F: FnOnce() -> T,
        T: Display,
    {
        if self.can_log {
            println!("[WRN] {}", f())
        }
    }

    /// Prints msg with [Err] header
    pub fn log_err<F, T>(&self, f: F)
    where
        F: FnOnce() -> T,
        T: Display,
    {
        if self.can_log {
            println!("[ERR] {}", f())
        }
    }
}
