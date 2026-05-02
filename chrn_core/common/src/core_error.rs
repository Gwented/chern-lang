// Definitions of all codebase wide errors

use std::path::Path;

use crate::reporter::diagnostic::Diagnostic;

/// General error enum for the entirety of the codebase to use. Everything can be converted back
/// into it so it can be treated just as any `Err()` would be but more valuable in detail.
pub enum CoreError {
    Config(ConfigLoadError),
    Script(ScriptError),
    Serial(SerialError),
}

impl From<ConfigLoadError> for CoreError {
    fn from(cfg_err: ConfigLoadError) -> Self {
        CoreError::Config(cfg_err)
    }
}

impl From<ScriptError> for CoreError {
    fn from(script_err: ScriptError) -> Self {
        CoreError::Script(script_err)
    }
}

impl From<SerialError> for CoreError {
    fn from(serial_err: SerialError) -> Self {
        CoreError::Serial(serial_err)
    }
}

/// Error type for ChrnConfigLoader
// More like startup error now
#[derive(Debug)]
pub enum ConfigLoadError {
    Unclosed(Diagnostic),
    Module(Diagnostic),
    IO(std::io::Error),
}

impl From<std::io::Error> for ConfigLoadError {
    fn from(err: std::io::Error) -> Self {
        ConfigLoadError::IO(err)
    }
}

#[derive(Debug)]
pub enum ScriptError {
    Parser(Vec<Diagnostic>),
    Semantic(Vec<Diagnostic>),
    IO(std::io::Error),
}

impl From<std::io::Error> for ScriptError {
    fn from(err: std::io::Error) -> Self {
        ScriptError::IO(err)
    }
}

#[derive(Debug)]
pub enum SerialError {
    Lexer(Vec<Diagnostic>),
    Parser(Vec<Diagnostic>),
    IO(std::io::Error),
}

impl From<std::io::Error> for SerialError {
    fn from(err: std::io::Error) -> Self {
        SerialError::IO(err)
    }
}

// Naming naming naming namingnamingnamign
/// Preset of error messages to reduce code duplication for file io errors. Returns a `Some` type
/// with a preset error. Returns `None` if no present is available which allows for the caller to
/// choose whether to use built-in error messages rather than doing it by default.
pub fn form_string_from_io_err(err: &std::io::Error, path: &Path) -> Option<String> {
    match err.kind() {
        std::io::ErrorKind::NotFound => {
            // Is file too specific!
            // Probably I don't know
            Some(format!("No file found in path \"{}\"", path.display()))
        }

        std::io::ErrorKind::IsADirectory => {
            Some(format!("The path \"{}\" is a directory", path.display()))
        }
        std::io::ErrorKind::PermissionDenied => Some(format!(
            "Cannot read file \"{}\" due to a lack of read permissions",
            path.display()
        )),
        std::io::ErrorKind::AlreadyExists => {
            Some(format!("The path \"{}\" already exists", path.display()))
        }
        std::io::ErrorKind::NotADirectory => Some(format!(
            "The path \"{}\" is not a directory",
            path.display()
        )),
        // std::io::ErrorKind::ResourceBusy => todo!(),
        // std::io::ErrorKind::Interrupted => todo!(),
        // std::io::ErrorKind::ExecutableFileBusy => todo!(),
        // std::io::ErrorKind::OutOfMemory => todo!(),
        _ => None,
    }
}
