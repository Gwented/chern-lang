// Will probably be in common
use std::{
    fs,
    hash::{self, Hasher},
    io::{self, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    ptr,
};

use chrn_utils::core_error;

const MAX_TEMP_FILE_CREATION_ATTEMPTS: u16 = 500;

pub(crate) fn make_canon(path: &Path) -> Result<PathBuf, String> {
    match path.canonicalize() {
        Ok(p) => Ok(p),
        Err(e) => Err(core_error::form_string_from_io_err(&e, path).unwrap_or(e.to_string())),
    }
}
