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

//TODO: Maybe some optimizations here if possible since who knows how big the file is, but at the
//same time we need to preserve the file then write it back into the file, which is bound to take a
//lot of memory possibly.
/// Writes `to_write` into the file `dest` at the front.
///
/// Loads all of `dest` into memory, appends to `to_write`, then writes all of it
/// into `dest`.
pub(crate) fn write_bytes_front(dest: &Path, to_write: &[u8]) -> Result<(), io::Error> {
    let mut new_data = to_write.to_vec();
    let mut file = fs::File::open(dest)?;
    file.read_to_end(&mut new_data)?;

    let mut new_file = fs::File::create(dest)?;
    new_file.write_all(&new_data)?;
    Ok(())
}

/// Writes `to_write` into the file `dest` at the front.
///
/// Creates a file with a random name, puts `to_write` inside, streams `dest`'s contents into it,
/// removes the original file, then renames the random file name to the original file name `dest` had.
///
/// NOTE: Internally creates custom errors using the same `std::io::Error` type where needed.
pub(crate) fn write_bytes_front_stream(
    dest_path: &Path,
    front_bytes: &[u8],
) -> Result<(), io::Error> {
    // -- SETUP --
    let dest_dir = match dest_path.parent() {
        Some(p) => p,
        None => {
            // Is this ok?
            let err_msg = format!("Could not get dir of destination {}", dest_path.display());
            let custom_err = io::Error::new(io::ErrorKind::InvalidInput, err_msg);
            return Err(custom_err.into());
        }
    };

    // Setting current dir of stream file to the destination's dir so that the operation happens in
    // the same directory.
    let mut stream_file_path = dest_dir.to_path_buf();

    // If it fails a certain amount of times attempts are stopped
    let mut created_temp_name = false;

    let mut hasher = hash::DefaultHasher::new();

    //DO NOT ASK ABOUT THIS
    for _ in 0..MAX_TEMP_FILE_CREATION_ATTEMPTS {
        ptr::hash(front_bytes.as_ptr(), &mut hasher);
        let hashed_val = hasher.finish();
        // File name of streamed file

        // Extra allocation but worth it for clarity
        stream_file_path.push(format!("{hashed_val}chrnstream"));

        // Checks if generated name exists before trying
        if !stream_file_path.exists() {
            created_temp_name = true;
            break;
        }
        stream_file_path.pop();
    }

    if !created_temp_name {
        let err_msg = format!(
            "Attempted to create temp file name to stream into {MAX_TEMP_FILE_CREATION_ATTEMPTS} times and failed.\nIf the issue persists consider using `--in-memory`"
        );
        let custom_err = io::Error::new(io::ErrorKind::InvalidFilename, err_msg);
        return Err(custom_err);
    }

    // -- FILE OPERATIONS --

    // File that will be renamed after being streamed into
    let stream_file = fs::File::create_new(&stream_file_path)?;

    // 32KB is probably fine?
    let mut stream_writer = BufWriter::with_capacity(32768, stream_file);

    // Writing given bytes to duplicate before streaming into file
    stream_writer.write_all(front_bytes)?;

    let dest_file = fs::File::open(dest_path)?;
    let mut dest_reader = BufReader::new(dest_file);

    let mut buffer = [0u8; 32768];
    // Ok but what about security what what if the the file has has ! an excessive amount?
    // Ok but how about no
    loop {
        let read = dest_reader.read(&mut buffer)?;

        if read == 0 {
            break;
        }

        stream_writer.write_all(&buffer[..read])?;
    }
    stream_writer.flush()?;

    fs::remove_file(dest_path)?;
    fs::rename(stream_file_path, dest_path)?;

    Ok(())
}
