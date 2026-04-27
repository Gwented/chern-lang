use std::path::{Path, PathBuf};

use crate::span::Span;

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
#[derive(Debug)]
pub enum Area {
    ConfigLoad,
    Script,
    Serial,
}

#[derive(Debug)]
pub struct Diagnostic {
    pub path: PathBuf,
    pub core_msg: String,
    pub span: Option<Span>,
    pub fmtted_diag: String,
    // pub help: Option<String>,
    pub area: Area,
}

impl Diagnostic {
    // Maybe just req PathBuf
    pub fn new(
        path: &Path,
        span: Option<Span>,
        core_msg: String,
        fmtted_diag: String,
        // help: Option<String>,
        area: Area,
    ) -> Diagnostic {
        Diagnostic {
            path: path.to_path_buf(),
            span,
            core_msg,
            fmtted_diag,
            // help,
            area,
        }
    }
}
