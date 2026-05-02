use std::{
    fs::{self, File},
    io,
    path::Path,
};

//TEST:
pub fn fopen(path: &Path) -> Result<File, String> {
    match fs::File::open(path) {
        Ok(_) if path.is_dir() => {
            let msg = format!("The path \"{}\" is a directory", path.display());
            Err(msg)
        }
        Ok(f) => Ok(f),
        Err(e) => match e.kind() {
            io::ErrorKind::NotFound => {
                let msg = format!("No file found in path \"{}\"", path.display());
                Err(msg)
            }
            io::ErrorKind::IsADirectory => {
                let msg = format!("The path \"{}\" is a directory", path.display());
                Err(msg)
            }
            io::ErrorKind::PermissionDenied => {
                let msg = format!(
                    "The file \"{}\" does not have read permissions enabled",
                    path.display()
                );

                Err(msg)
            }
            e => {
                let msg = format!("Process exited unsuccessfully.\n{e}");
                Err(msg)
            }
        },
    }
}
