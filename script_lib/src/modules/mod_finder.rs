use std::{
    ffi::{OsStr, OsString},
    os::unix::ffi::{OsStrExt, OsStringExt},
    path::PathBuf,
};

use common::{
    intern::Intern,
    symbols::{NameId, PathId},
};

use crate::modules::Module;

pub struct ModuleFinder<'a> {
    pub src_bytes: &'a [u8],
    pos: usize,
    end: usize,
}

impl ModuleFinder<'_> {
    pub fn new(
        src_bytes: &[u8],
        script_start: usize,
        serial_start: Option<usize>,
    ) -> ModuleFinder<'_> {
        ModuleFinder {
            src_bytes,
            pos: script_start,
            end: serial_start.unwrap_or(src_bytes.len()),
        }
    }

    pub fn collect_imports(&mut self, interner: &mut Intern) -> Vec<PathId> {
        let mut imports: Vec<PathId> = Vec::new();

        loop {
            self.skip_until_i();

            if self.pos >= self.end && self.peek() == b'\0' {
                break;
            }

            let ch = self.peek();

            match ch {
                // b'b' => if self.read_id(interner).is_some() {},
                b'i' => {
                    if self.is_import() {
                        let name_id = self.read_import_name(interner);
                        imports.push(name_id);
                    } else {
                        self.advance();
                    }
                }
                b'"' => {
                    self.skip_quotes();
                }
                b'/' => {
                    if self.peek_ahead(1) == b'/' {
                        self.skip(2);
                        self.handle_comment();
                    } else if self.peek_ahead(1) == b'*' {
                        self.skip(2);
                        self.handle_multi_comment();
                    } else {
                        self.advance();
                    }
                }
                _ => {
                    self.advance();
                }
            }
        }

        imports
    }

    fn handle_comment(&mut self) {
        while self.peek() != b'\n' {
            self.advance();
        }
    }

    /// Assumes first quote was skipped
    fn read_import_name(&mut self, interner: &mut Intern) -> PathId {
        let start = self.pos;

        while self.pos < self.src_bytes.len() {
            match self.peek() {
                b'\\' => {
                    self.advance();

                    // Um
                    if let Some(_) = self.read_escape() {
                    } else {
                    }
                }
                b'"' => {
                    self.advance();
                    break;
                }
                _ => {
                    self.advance();
                }
            }
        }
        let end = self.pos - 1;

        //FIX:
        let os_str = OsStr::from_bytes(&self.src_bytes[start..end]);
        let import_name = PathBuf::from(os_str);

        let path_id = PathId::new(interner.intern_path(&import_name));

        path_id
    }

    fn skip_quotes(&mut self) {
        while self.pos < self.src_bytes.len() {
            match self.peek() {
                b'\\' => {
                    self.advance();

                    self.read_escape();
                }
                b'"' => {
                    self.advance();
                    break;
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn read_escape(&mut self) -> Option<char> {
        match self.peek() {
            b'n' => {
                self.advance();
                Some('\n')
            }
            b'r' => {
                self.advance();
                Some('\r')
            }
            b't' => {
                self.advance();
                Some('\t')
            }
            b'\\' => {
                self.advance();
                Some('\\')
            }
            b'0' => {
                self.advance();
                Some('\0')
            }
            b'\'' => {
                self.advance();
                Some('\'')
            }
            b'"' => {
                self.advance();
                Some('"')
            }
            b'x' => {
                self.advance();
                let mut val: u8 = 0;
                let mut count = 0;

                while count < 2 {
                    let c = self.peek();

                    let digit = match c {
                        b'0'..=b'9' => c - b'0',
                        b'a'..=b'f' => c - b'a' + 10,
                        b'A'..=b'F' => c - b'A' + 10,
                        _ => break,
                    };

                    val = (val << 4) | digit;
                    self.advance();
                    count += 1;
                }

                if count == 2 {
                    let next = self.peek();
                    if matches!(next, b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F') {
                        None
                    } else {
                        Some(val as char)
                    }
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn is_import(&mut self) -> bool {
        let start = self.pos;

        while self.pos < self.src_bytes.len() && self.peek().is_ascii_alphabetic() {
            self.advance();
        }

        let end = self.pos;

        if &self.src_bytes[start..end] != b"import" {
            return false;
        }

        //FIX: Utf 98
        self.skip_whitespace();

        if self.peek() != b'"' {
            return false;
        }

        // Skipping first quote
        self.advance();
        true
    }

    fn handle_multi_comment(&mut self) {
        let mut depth = 1;

        while self.pos < self.src_bytes.len() && depth > 0 {
            if self.peek() == b'/' && self.peek_ahead(1) == b'*' {
                self.skip(1);
                depth += 1;
            } else if self.peek() == b'*' && self.peek_ahead(1) == b'/' {
                self.skip(2);
                depth -= 1;
            } else {
                self.advance();
            }
        }

        if depth > 0 {
            eprintln!("Could not find end of multi-line comment");
        }
    }

    fn skip(&mut self, dest: usize) {
        self.pos += dest;
    }

    fn peek(&self) -> u8 {
        self.src_bytes.get(self.pos).copied().unwrap_or(b'\0')
    }

    fn peek_ahead(&mut self, dest: usize) -> u8 {
        self.src_bytes
            .get(self.pos + dest)
            .copied()
            .unwrap_or(b'\0')
    }

    fn advance(&mut self) -> u8 {
        let b = self.peek();
        self.pos += 1;
        b
    }

    fn skip_until_i(&mut self) {
        // Stopping at parts that may cause wrongful import reads
        while self.pos <= self.end
            && self.peek() != b'i'
            && self.peek() != b'"'
            && self.peek() != b'/'
        {
            self.advance();
        }
    }

    fn skip_whitespace(&mut self) {
        // Stopping at parts that may cause wrongful import reads
        while self.pos <= self.end && self.peek().is_ascii_whitespace() {
            self.advance();
        }
    }
}
