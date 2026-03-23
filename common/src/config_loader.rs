//FIXME: FIX REQUIRES END MESSAGE
use std::{
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
};

use crate::metadata::FileMetadata;

// TEST: Ignore this
const DEFINITION_SIZE: usize = 4;
const MAX_READ: usize = 1_000_000;

// More inclusive name
//TEST: Suspicious lifetime
pub struct ConfigLoader<'a, R: Read> {
    // Configuration file path
    path: &'a Path,
    handle: BufReader<R>,
    pos: usize,
    lines_read: usize,
}

//NOTE: This forces paths to be given, but if the chern file itself doesn't have a path given
//then the language doesn't work anyways. May leave as is.
impl<R: Read> ConfigLoader<'_, R> {
    pub fn new(path: &Path, handle: R) -> ConfigLoader<'_, R> {
        ConfigLoader {
            path,
            handle: BufReader::new(handle),
            pos: 0,
            lines_read: 1,
        }
    }

    /// Returns a Success value of the bytes to Lex, the offset of where to start lexing if an
    /// `@def` is present, and the offset of where to start reading the serialized data if an
    /// `@def` is present. Returns a String upon failure that has the error reason inside.
    pub fn load_config(&mut self) -> Result<FileMetadata, String> {
        // Doesn't NEED definition but will error if declared and not closed
        let mut requires_end = false;
        let mut saw_quotes = false;

        let mut lex_start = 0;

        self.handle.fill_buf().or_else(|e| {
            Err(format!(
                "internal error: Failed to fill buffer to read configuration file\n{e}"
            ))
        })?;

        while let Some(b) = self.peek() {
            if b == b'\0' {
                break;
            }

            match b {
                b'"' | b'\'' => {
                    // Even though this can't fail
                    let quote = self.advance().unwrap_or(b'\0');

                    let start_line = self.lines_read;

                    // Is there a reason for lines_read to be printed if there are multiple quotes?
                    // When are there ever NOT multiple quotes if it's in a serialized file?
                    if self.read_quotes(quote).is_err() {
                        let note = if saw_quotes {
                            "\nnote: There are other quotes within the file so the line given could be incorrect"
                        } else {
                            ""
                        };

                        let msg = format!(
                            "Found unclosed quotes at line {} which reached <eof>{}",
                            start_line, note
                        );

                        return Err(msg);
                    }

                    saw_quotes = true;
                }
                b'/' => {
                    self.advance();

                    if self.peek() == Some(b'/') {
                        self.advance();
                        self.handle_comment();
                    } else if self.peek() == Some(b'*') {
                        self.advance();
                        self.handle_multi_comment()?;
                    }
                }
                b'@' => {
                    // Helper boolean
                    let can_check = if self.pos + DEFINITION_SIZE < self.handle.buffer().len() {
                        true
                    } else {
                        false
                    };

                    if requires_end && !can_check {
                        break;
                    } else if requires_end
                        && &self.handle.buffer()[self.pos..self.pos + DEFINITION_SIZE] == b"@end"
                    {
                        let serial_start = self.pos + DEFINITION_SIZE;

                        return Ok(FileMetadata::new(
                            PathBuf::from(self.path),
                            self.handle.buffer()[..self.pos + DEFINITION_SIZE].to_vec(),
                            lex_start,
                            Some(serial_start),
                        ));
                    }

                    if !requires_end && !can_check {
                        break;
                    } else if !requires_end
                        && &self.handle.buffer()[self.pos..self.pos + DEFINITION_SIZE] == b"@def"
                    {
                        requires_end = true;
                        lex_start = self.pos;
                    }

                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }
        // TODO: Assert this...

        // Case of no @def and no @end which requires a '0' return since hte entire file should be
        // read. This does not mean it is correct, it only means the read limit wasn't reached.
        if !requires_end {
            // NOTE: May use lifetimes...
            Ok(FileMetadata::new(
                PathBuf::from(self.path),
                self.handle.buffer()[..self.pos].to_vec(),
                lex_start,
                // Some or None
                None,
            ))
        } else {
            let msg = format!(
                "Could not find `@end` after `@def` from path \"{}\"",
                self.path.display()
            );

            Err(msg)
        }
    }

    /// Returns a result instead of an option because if there are unclosed quotes and this method
    /// fails, it would need return a Some value which DOESN'T represent a failure, making it
    /// misleading.
    // TODO: LEXER SHOULD ALSO HANDLE THIS ALONE
    fn read_quotes(&mut self, quote_type: u8) -> Result<(), ()> {
        let mut read_bytes = 0;

        while let Some(b) = self.peek() {
            read_bytes += 1;

            if read_bytes == MAX_READ {
                return Err(());
            }

            match b {
                b'\\' => {
                    self.skip(2);
                }
                b if b == quote_type => {
                    self.advance();
                    return Ok(());
                }
                _ => {
                    self.advance();
                }
            }
        }

        Err(())
    }

    fn handle_comment(&mut self) {
        while let Some(b) = self.peek()
            && b != b'\n'
        {
            self.advance();
        }
    }

    fn handle_multi_comment(&mut self) -> Result<(), String> {
        let mut depth = 1;
        let comment_start = self.lines_read;

        while let Some(current_byte) = self.peek()
            && depth > 0
        {
            //TODO: Simplify this
            if let Some(next_byte) = self.peek_ahead(1) {
                if current_byte == b'/' && next_byte == b'*' {
                    depth += 1;
                    self.skip(2);
                } else if current_byte == b'*' && next_byte == b'/' {
                    depth -= 1;
                    self.skip(2);
                } else {
                    self.advance();
                }
            } else {
                break;
            }
        }

        if depth > 0 {
            return Err(format!(
                "Found unclosed multi-line comment in configuration file which started at line {}",
                comment_start
            ));
        }

        Ok(())
    }

    fn skip(&mut self, dest: usize) {
        self.pos += dest;
    }

    fn advance(&mut self) -> Option<u8> {
        let b = self.peek();

        if b == Some(b'\n') {
            self.lines_read += 1;
        }
        self.pos += 1;

        b
    }

    fn peek_ahead(&mut self, dest: usize) -> Option<u8> {
        self.handle.buffer().get(self.pos + dest).copied()
    }

    fn peek(&mut self) -> Option<u8> {
        self.handle.buffer().get(self.pos).copied()
    }
}
