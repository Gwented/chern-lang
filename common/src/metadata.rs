use std::{io::IsTerminal, path::PathBuf};

#[derive(Debug)]
pub struct ChernMetadata {
    /// Path to chrn config file
    pub path: PathBuf,
    /// Bytes from chrn config file
    pub src_bytes: Vec<u8>,
    /// Amount of \n within config file so binary search can be done by error reporter
    pub new_lines: Vec<usize>,
    /// The lexers start which can be different depending on if @def is used
    pub lex_start: usize,
    /// The serial start which can be None if there is no serialized file within the config file
    pub serial_start: Option<usize>,
    /// For preventing ANSI in places where it would be destructive
    pub can_color: bool,
}

impl ChernMetadata {
    pub fn new(
        path: PathBuf,
        src_bytes: Vec<u8>,
        lex_start: usize,
        serial_start: Option<usize>,
    ) -> ChernMetadata {
        // Does this actually make a difference?
        let can_color = if std::io::stdout().is_terminal() && std::io::stderr().is_terminal() {
            true
        } else {
            false
        };

        ChernMetadata {
            path,
            new_lines: Vec::new(),
            src_bytes,
            lex_start,
            serial_start,
            //TODO: Could be env var
            can_color,
        }
    }
}
