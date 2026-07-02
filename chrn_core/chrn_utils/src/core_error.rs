// Definitions of all codebase wide errors

use std::path::Path;

use crate::{
    intern::Intern,
    source_map::{source_diagnostic::SourceDiagnostic, source_region::SourceRegionArena},
};

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
    General(SourceDiagnostic),
    //// Contains whether or not the program can continue after critical error
    // Critical(bool),
    IO(std::io::Error),
}
impl From<std::io::Error> for ConfigLoadError {
    fn from(err: std::io::Error) -> Self {
        ConfigLoadError::IO(err)
    }
}

// Should this be here?
/// Struct for carrying module data if `main` fails to be loaded within `extract_modules`.
pub struct ModuleInitError {
    pub region: Option<SourceRegionArena>,
    pub interner: Intern,
    pub cfg_err: ConfigLoadError,
}

impl ModuleInitError {
    pub fn new(
        region: Option<SourceRegionArena>,
        interner: Intern,
        cfg_err: ConfigLoadError,
    ) -> ModuleInitError {
        ModuleInitError {
            region,
            interner,
            cfg_err,
        }
    }
}

#[derive(Debug)]
pub enum ScriptError {
    Parser(Vec<SourceDiagnostic>),
    Semantic(Vec<SourceDiagnostic>),
    IO(std::io::Error),
}

impl From<std::io::Error> for ScriptError {
    fn from(err: std::io::Error) -> Self {
        ScriptError::IO(err)
    }
}

// Since it's external maybe this shouldn't exist, or should at least act more so as a compatibility
// layer?
#[derive(Debug)]
pub enum SerialError {
    Lexer(Vec<SourceDiagnostic>),
    Parser(Vec<SourceDiagnostic>),
    IO(std::io::Error),
}

impl From<std::io::Error> for SerialError {
    fn from(err: std::io::Error) -> Self {
        SerialError::IO(err)
    }
}

pub enum CriticalError {}

// Naming naming naming namingnamingnamign
/// Preset of error messages to reduce code duplication for file io errors. Returns a `Some` type
/// with a preset error. Returns `None` if no present is available.
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
            "Cannot read file \"{}\" due to not having read permissions",
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
