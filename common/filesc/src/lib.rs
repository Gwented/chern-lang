use std::{
    fs::{self, File},
    hash::{self, Hasher},
    io::{self, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    ptr,
};

use algoc::mixer;

const MAX_TEMP_FILE_CREATION_ATTEMPTS: u16 = 500;

/// Writes `to_write` into the file `dest` at the front.
///
/// Loads all of `dest` into memory, appends to `to_write`, then writes all of it
/// into `dest`.
pub fn write_bytes_front(dest: &Path, to_write: &[u8]) -> Result<(), io::Error> {
    let mut new_data = to_write.to_vec();
    let mut file = fs::File::open(dest)?;
    file.read_to_end(&mut new_data)?;

    let mut new_file = fs::File::create(dest)?;
    new_file.write_all(&new_data)?;
    Ok(())
}

/// Writes `to_write` into the file `dest` from the beginning.
///
/// Creates a file with a random name, puts `to_write` inside, streams `dest`'s contents into it,
/// removes the original file, then renames the random file name to the original file name `dest` had.
///
/// NOTE: Internally creates custom errors using the same `std::io::Error` type where needed.
pub fn write_bytes_front_stream(
    dest_path: &Path,
    front_bytes: &[u8],
    tmp_file_name_suffix: Option<&str>,
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

    // -- FILE OPERATIONS --

    // File that will be renamed after being streamed into
    let stream_file = create_unique_file(&mut stream_file_path, tmp_file_name_suffix)?;

    // 32KB is probably fine?
    let mut stream_writer = BufWriter::with_capacity(32768, stream_file);

    // Writing given bytes to duplicate before streaming into file
    stream_writer.write_all(front_bytes)?;

    let dest_file = File::open(dest_path)?;
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

/// Attempts to create a file that is gauranteed to have a unique file name inside the given `path`
/// `tmp_file_name_suffix` is appended if given, otherwise the file name is just the hash.
pub fn create_unique_file(
    path: &mut PathBuf,
    tmp_file_name_prefix: Option<&str>,
) -> Result<File, io::Error> {
    // If it fails a certain amount of times attempts are stopped
    let mut created_temp_name = false;

    let mut hasher = hash::DefaultHasher::new();
    ptr::hash(&hasher, &mut hasher);
    hasher.write_u32(std::process::id());

    for seed in 0..MAX_TEMP_FILE_CREATION_ATTEMPTS {
        hasher.write_u16(seed);
        let hash = hasher.finish();

        path.push(format!(
            "{}{:#x}.part",
            tmp_file_name_prefix.unwrap_or_default(),
            hash,
        ));

        if !path.exists() {
            created_temp_name = true;
            break;
        }
        path.pop();
    }

    if !created_temp_name {
        let err_msg = format!(
            "Attempted to create unique file name {MAX_TEMP_FILE_CREATION_ATTEMPTS} times and failed"
        );
        let custom_err = io::Error::new(io::ErrorKind::InvalidFilename, err_msg);
        return Err(custom_err);
    }

    File::create_new(&path)
}
