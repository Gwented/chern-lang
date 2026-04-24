use std::path::{Path, PathBuf};

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
    pub fmtted_msg: String,
    pub area: Area,
}

impl Diagnostic {
    pub fn new(path: &Path, fmtted_msg: String, area: Area) -> Diagnostic {
        Diagnostic {
            path: path.to_path_buf(),
            fmtted_msg,
            area,
        }
    }
}
