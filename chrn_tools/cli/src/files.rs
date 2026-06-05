use std::path::{Path, PathBuf};

use chrn_utils::core_error;

pub(crate) fn make_canon(path: &Path) -> Result<PathBuf, String> {
    match path.canonicalize() {
        Ok(p) => Ok(p),
        Err(e) => Err(core_error::form_string_from_io_err(&e, path).unwrap_or(e.to_string())),
    }
}
