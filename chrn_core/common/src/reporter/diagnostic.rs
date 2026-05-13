use std::path::{Path, PathBuf};

use crate::span::Span;

/// This exists in case other methods or fields are considered, but is just a Vec<Diagnostic>
/// wrapper as of right now
#[derive(Debug)]
pub struct Reporter {
    pub diags: Vec<Diagnostic>,
}

impl Reporter {
    pub fn new() -> Reporter {
        Reporter { diags: Vec::new() }
    }
}

/// Although there are error types that say where the error came from, all of `CoreError` needs to
/// still returns `Diagnostic` as a vector, which could have other areas inside of it, making this
/// serve as persistent metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Area {
    ConfigLoad,
    Script,
    Serial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Error,
    Note,
    Warn,
}

// Would 2 diagnostics need to be produced where one has error and otherh as help related to it?
// Um.
/// Generic diagnostic struct
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// The path origin of the diagnostic
    pub path: PathBuf,
    /// The base message attached to the diagnostic
    pub core_msg: String,
    /// The span of where the message took place
    pub span: Option<Span>,
    /// The fully formatted error that is attached to the diagnostic which avoids having to in-line
    /// creation outside of one source of internal truth.
    pub fmtted_diag: String,
    pub help: Option<String>,
    // level: DiagnosticLevel,
    // pub help: Option<String>,
    /// Data for which stage of what compiler the diagnostic was emitted from
    pub area: Area,
}

impl Diagnostic {
    // Maybe just req PathBuf
    // Maybe just require PathBuf PLEASE
    pub fn new(
        path: &Path,
        core_msg: String,
        span: Option<Span>,
        help: Option<String>,
        fmtted_diag: String,
        area: Area,
    ) -> Diagnostic {
        Diagnostic {
            path: path.to_path_buf(),
            span,
            core_msg,
            fmtted_diag,
            help,
            area,
        }
    }
}
