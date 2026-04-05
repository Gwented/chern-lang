// Definitions of all codebase wide errors

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

/// Error type for ChernConfigLoader
#[derive(Debug)]
pub enum ConfigLoadError {
    Unclosed(String),
    Module(String),
    IO(std::io::Error),
}

impl From<std::io::Error> for ConfigLoadError {
    fn from(err: std::io::Error) -> Self {
        ConfigLoadError::IO(err)
    }
}

#[derive(Debug)]
pub enum ScriptError {
    Parser(Vec<String>),
    Semantic(Vec<String>),
    IO(std::io::Error),
}

impl From<std::io::Error> for ScriptError {
    fn from(err: std::io::Error) -> Self {
        ScriptError::IO(err)
    }
}

#[derive(Debug)]
pub enum SerialError {
    Lexer(),
    Parser(),
    IO(std::io::Error),
}

impl From<std::io::Error> for SerialError {
    fn from(err: std::io::Error) -> Self {
        SerialError::IO(err)
    }
}
