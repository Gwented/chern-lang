use std::{io::IsTerminal, path::PathBuf};

#[derive(Debug)]
pub struct FileMetadata {
    pub path: PathBuf,
    pub src_bytes: Vec<u8>,
    pub new_lines: Vec<usize>,
    pub lex_start: usize,
    pub serial_start: usize,
    pub can_color: bool,
}

impl FileMetadata {
    pub fn new(
        path: PathBuf,
        src_bytes: Vec<u8>,
        lex_start: usize,
        serial_start: usize,
    ) -> FileMetadata {
        // Does this actually make a difference?
        let can_color = if std::io::stdout().is_terminal() && std::io::stderr().is_terminal() {
            true
        } else {
            false
        };

        FileMetadata {
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
