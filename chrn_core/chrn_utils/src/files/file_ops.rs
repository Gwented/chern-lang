use std::{
    fs::{self, File},
    io,
    path::Path,
};

// This seems fit for common/ since it's a non-specific msg helper
//TEST:
/// Convenience function for opening files
///
/// On `Ok` returns file
/// On `Err` returns (std::io::Error, Conventional Message) which allows for the deduplication of
/// making error messages in regards to IO errors.
pub fn fopen(path: &Path) -> Result<File, (io::Error, String)> {
    match fs::File::open(path) {
        Ok(_) if path.is_dir() => {
            let msg = format!("The path \"{}\" is a directory", path.display());
            let err = io::Error::new(
                io::ErrorKind::IsADirectory,
                "The path \"{}\" is a directory",
            );
            Err((err, msg))
        }
        Ok(f) => Ok(f),
        Err(e) => {
            let msg = match e.kind() {
                io::ErrorKind::NotFound => {
                    format!("No file found in path \"{}\"", path.display())
                }
                io::ErrorKind::IsADirectory => {
                    format!("The path \"{}\" is a directory", path.display())
                }
                io::ErrorKind::PermissionDenied => {
                    format!(
                        "The file \"{}\" does not have read permissions enabled",
                        path.display()
                    )
                }
                e => e.to_string(),
            };
            Err((e, msg))
        }
    }
}
