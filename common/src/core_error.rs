// #include <stdio.h> #include <stdlib.h> typedef struct {} Floor; int main() { return 0; }

/// General error enum for the entirety of the codebase to use. Everything can be converted back
/// into it so it can be treated just as any Err() would be but more valuable in detail.
pub enum CoreError {
    Config(ConfigLoadError),
}

#[derive(Debug)]
pub enum ConfigLoadError {
    UnclosedQuotes(String),
    UnclosedDef(String),
    IO(std::io::Error),
}

// I don't think this is too much since core error is core of the program meaning it will be
// everywhere. Or should be at least.
impl From<ConfigLoadError> for CoreError {
    fn from(cfg_err: ConfigLoadError) -> Self {
        CoreError::Config(cfg_err)
    }
}

impl From<std::io::Error> for ConfigLoadError {
    fn from(err: std::io::Error) -> Self {
        ConfigLoadError::IO(err)
    }
}
