/// Logger!!
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
        T: std::fmt::Display,
    {
        if self.can_log {
            println!("[DBG] {}", f())
        }
    }

    /// Prints msg with [WRN] header
    pub fn log_warn<F, T>(&self, f: F)
    where
        F: FnOnce() -> T,
        T: std::fmt::Display,
    {
        if self.can_log {
            println!("[WRN] {}", f())
        }
    }

    /// Prints msg with [ERR] header
    pub fn log_err<F, T>(&self, f: F)
    where
        F: FnOnce() -> T,
        T: std::fmt::Display,
    {
        if self.can_log {
            println!("[ERR] {}", f())
        }
    }
}
