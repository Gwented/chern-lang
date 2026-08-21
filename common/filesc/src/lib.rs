use std::{
    fs::{self, File},
    hash::{self, Hasher},
    io::{self, BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
    ptr,
};

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
    tmp_file_name_suffix: Option<&str>,
) -> Result<File, io::Error> {
    // If it fails a certain amount of times attempts are stopped
    let mut created_temp_name = false;

    let mut hasher = hash::DefaultHasher::new();

    for _ in 0..MAX_TEMP_FILE_CREATION_ATTEMPTS {
        ptr::hash(&hasher, &mut hasher);
        let hashed_name = hasher.finish();

        // Extra allocation but worth it for clarity
        path.push(format!(
            "{hashed_name}{}",
            tmp_file_name_suffix.unwrap_or_default()
        ));

        // Checks if generated name exists before trying
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

    // -- FILE OPERATIONS --
    File::create_new(&path)
}

// Doing a bit too much or no?
// UM
// Maybe we can drop the formatting part
// Also please rename it from fopen

// / Convenience function that opens a file and ensures it's not a directory before returning it,
// / which is not the default behavior unless explicitly checked.
// /
// / Returns with formatted error messages for a select few kinds of `std::io::Error`s.
// /
// / On `Ok` returns file
// / On `Err` returns (std::io::Error, Conventional Message) which allows for the deduplication of
// / making error messages in regards to IO errors.
// pub fn fopen(path: &Path) -> Result<File, (io::Error, String)> {
//     match fs::File::open(path) {
//         Ok(_) if path.is_dir() => {
//             let msg = format!("The path \"{}\" is a directory", path.display());
//             let err = io::Error::new(
//                 io::ErrorKind::IsADirectory,
//                 "The path \"{}\" is a directory",
//             );
//             Err((err, msg))
//         }
//         Ok(f) => Ok(f),
//         Err(e) => {
//             let msg = match e.kind() {
//                 io::ErrorKind::NotFound => {
//                     format!("No file found in path \"{}\"", path.display())
//                 }
//                 io::ErrorKind::IsADirectory => {
//                     format!("The path \"{}\" is a directory", path.display())
//                 }
//                 io::ErrorKind::PermissionDenied => {
//                     format!(
//                         "The file \"{}\" does not have read permissions enabled",
//                         path.display()
//                     )
//                 }
//                 e => e.to_string(),
//             };
//             Err((e, msg))
//         }
//     }
// }
