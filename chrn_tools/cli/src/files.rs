use std::{
    fs,
    io::{BufWriter, Read, Write},
    path::{Path, PathBuf},
};

use chrn_utils::core_error;

pub(crate) fn make_canon(path: &Path) -> Result<PathBuf, String> {
    match path.canonicalize() {
        Ok(p) => Ok(p),
        Err(e) => Err(core_error::form_string_from_io_err(&e, path).unwrap_or(e.to_string())),
    }
}

//TODO: Maybe some optimizations here if possible since who knows how big the file is, but at the
//same time we need to preserve the file then write it back into the file, which is bound to take a
//lot of memory possibly.
/// Writes `to_write` into the file `dest` at the front.
///
/// Loads all of `dest` into memory, appends all the data into `to_write`, then writes all of it
/// into the file.
pub(crate) fn write_bytes_front(dest: &Path, to_write: &[u8]) -> Result<(), std::io::Error> {
    let mut new_data = to_write.to_vec();
    let mut file = fs::File::open(dest)?;
    file.read_to_end(&mut new_data)?;

    let mut new_file = fs::File::create(dest)?;
    new_file.write_all(&new_data)?;
    new_file.flush()?;
    Ok(())
}
