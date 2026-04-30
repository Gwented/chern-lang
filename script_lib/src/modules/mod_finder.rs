use std::{ffi::OsStr, path::PathBuf, str::FromStr};

use chrn_utils::{
    id_types::{InternedId, PathId},
    intern::Intern,
};
use common::{
    chrn_settings::ChernSettings,
    core_error::{self, ConfigLoadError},
    reporter::{
        self,
        diagnostic::{Area, Diagnostic},
    },
    span::Span,
};

use crate::modules::{Bind, Import};

//WARN: There is no information given to the module finder for what path the imports are actually
//being collected from.
pub struct ModuleFinder<'a> {
    src_bytes: &'a [u8],
    settings: &'a ChernSettings,
    pos: usize,
    end: usize,
}

impl ModuleFinder<'_> {
    pub fn new<'a>(
        //TODO: Need to store beg
        src_bytes: &'a [u8],
        settings: &'a ChernSettings,
        script_start: usize,
        serial_start: Option<usize>,
    ) -> ModuleFinder<'a> {
        ModuleFinder {
            src_bytes,
            settings,
            pos: script_start,
            end: serial_start.unwrap_or(src_bytes.len()),
        }
    }

    /// Returns a tuple of `Bind` and any imports found on Success.
    pub(crate) fn collect_imports(
        &mut self,
        interner: &mut Intern,
    ) -> Result<(Option<Bind>, Vec<Import>), ConfigLoadError> {
        let mut imports: Vec<Import> = Vec::new();
        let mut bind: Option<Bind> = None;

        loop {
            //FIX: Does not account for "e#" I think I don't know
            self.skip_until_important();

            if self.pos >= self.end && self.peek() == b'\0' {
                break;
            }

            let ch = self.peek();

            match ch {
                b'i' => {
                    if self.is_import() {
                        let import = self.parse_import(interner)?;
                        imports.push(import);
                    }
                }
                b'b' => {
                    if self.is_bind() {
                        bind = Some(self.parse_bind(interner)?);
                    }
                    self.advance();
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

        Ok((bind, imports))
    }

    /// Assumes the starting point is at the start quote
    fn parse_import(&mut self, interner: &mut Intern) -> Result<Import, ConfigLoadError> {
        self.advance();
        let start = self.pos;

        while self.pos < self.src_bytes.len() {
            match self.peek() {
                b'"' => {
                    self.advance();
                    break;
                }
                _ => {
                    self.advance();
                }
            }
        }

        // - 1 to include quotes since that happens in the lexer. No other reason.
        let end = self.pos - 1;

        let path_span = Span::new(start - 1, end);
        let path_buf = self.create_pathbuf(&self.src_bytes[start..end])?;

        let import_path = match path_buf.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                let core_msg =
                    core_error::form_string_from_io_err(&e, &path_buf).unwrap_or(e.to_string());

                let ln_data =
                    reporter::form_err_diag(self.src_bytes, &[path_span], self.settings.can_color);

                let fmtted_diag = reporter::standardize_err(
                    &core_msg,
                    &ln_data,
                    "",
                    &path_buf,
                    self.settings.can_color,
                );

                let diag = Diagnostic::new(
                    &path_buf,
                    core_msg,
                    Some(path_span),
                    fmtted_diag,
                    Area::ConfigLoad,
                );

                return Err(ConfigLoadError::Module(diag));
            }
        };

        // We could check for an alias here too in case the file name is invalid and needs an alias
        let file_name = match import_path.file_prefix().map(|n| n.to_str()) {
            Some(Some(n)) => n,
            e => todo!("{e:?}"),
        };

        let name_id = InternedId::new(interner.intern(&file_name));
        let path_id = PathId::new(interner.intern_path(&import_path));

        let alias_id: Option<InternedId> = if self.is_as() {
            self.skip_whitespace();
            Some(self.read_id(interner))
        } else {
            None
        };

        Ok(Import::new(name_id, path_id, path_span, alias_id))
    }

    //WARN: Will be placed in different module
    fn create_pathbuf(&self, slice: &[u8]) -> Result<PathBuf, ConfigLoadError> {
        if cfg!(unix) {
            #[cfg(unix)]
            {
                use std::os::unix::ffi::OsStrExt;
                let os_str = OsStr::from_bytes(slice);
                return Ok(PathBuf::from(os_str));
            }
            // NOTE: This may be done differently but this remains a basic utf-8 check for now
        } else if cfg!(windows) {
            // #[cfg(windows)]
            // {
            //     use std::os::windows::ffi::OsStrExt;
            //     let os_str = OsStr::from_wide(slice);
            //
            //     return Ok(PathBuf::from(slice));
            // }
            match str::from_utf8(slice) {
                Ok(s) => return Ok(PathBuf::from_str(&s).expect("Uh")),
                Err(_) => todo!(),
            }
        }

        match str::from_utf8(slice) {
            Ok(s) => Ok(PathBuf::from_str(&s).expect("Infailable")),
            Err(_) => {
                todo!()
            }
        }
    }

    fn parse_bind(&mut self, interner: &mut Intern) -> Result<Bind, ConfigLoadError> {
        // skipping "
        self.advance();
        let start = self.pos;

        while self.pos < self.src_bytes.len() {
            match self.peek() {
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

        // Um uh
        let path_buf = self.create_pathbuf(&self.src_bytes[start..end]).unwrap();
        let path_span = Span::new(start - 1, end);

        let bind_path = match path_buf.canonicalize() {
            Ok(p) => p,
            Err(_) => todo!(),
        };

        let path_id = PathId::new(interner.intern_path(&bind_path));

        Ok(Bind::new(path_id, path_span))
    }

    fn read_id(&mut self, interner: &mut Intern) -> InternedId {
        let start = self.pos;

        while self.pos < self.src_bytes.len() && self.peek_char().is_alphanumeric()
            || self.peek() == b'_'
        {
            self.advance_char();
        }

        let end = self.pos;

        // Enforces utf-8 but module paths themselves don't need to be valid utf-8, am I
        // hallucinating?
        let id_str = str::from_utf8(&self.src_bytes[start..end])
            .expect("Cannot fail due to loop only accepting valid UTF-8 characters.");

        let id = interner.intern(&id_str);

        InternedId::new(id)
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

        true
    }

    // WARN: Suspicipus
    fn is_as(&mut self) -> bool {
        self.skip_whitespace();

        if self.pos + 2 < self.src_bytes.len() && &self.src_bytes[self.pos..self.pos + 2] == b"as" {
            self.skip(2);
            return true;
        }

        false
    }

    fn is_bind(&mut self) -> bool {
        let start = self.pos;

        while self.pos < self.src_bytes.len() && self.peek().is_ascii_alphabetic() {
            self.advance();
        }

        let end = self.pos;

        if &self.src_bytes[start..end] != b"bind" {
            return false;
        }

        //FIX: Utf 98
        self.skip_whitespace();

        if self.peek() != b'"' {
            return false;
        }

        true
    }

    fn handle_comment(&mut self) {
        while self.peek() != b'\n' {
            self.advance();
        }
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

    fn peek_char(&mut self) -> char {
        let b = self.peek();

        if b <= 127 {
            return b as char;
        }

        let chunk = &self.src_bytes[self.pos..];

        // Should be a test for this this is suspicious
        // Lazy evaluation to avoid utf-8 checking entire self.bytes
        std::str::from_utf8(chunk)
            .ok()
            .and_then(|c| c.chars().next())
            .unwrap_or('\0')
    }

    fn advance_char(&mut self) -> char {
        let ch = self.peek_char();

        self.pos += ch.len_utf8();

        ch
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

    fn skip_until_important(&mut self) {
        // Stopping at parts that may cause wrongful import reads
        while self.pos <= self.end
            && self.peek() != b'i'
            && self.peek() != b'b'
            && self.peek() != b'"'
            && self.peek() != b'/'
        {
            self.advance();
        }
    }

    fn skip_whitespace(&mut self) {
        while self.peek().is_ascii_whitespace() {
            self.advance();
        }

        while self.peek_char().is_whitespace() {
            self.advance_char();
        }
    }
}
