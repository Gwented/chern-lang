use std::{ffi::OsStr, path::PathBuf, str::FromStr};

use chrn_utils::{
    chrn_settings::ChrnSettings,
    core_error::{self, ConfigLoadError},
    id_types::InternedId,
    intern::Intern,
    source_map::{
        source_diagnostic::{AnnotationKind, DiagnosticLevel, SourceDiagnostic},
        source_region::SourceRegion,
        source_span::SourceSpan,
    },
};

use crate::modules::{Bind, Import, ImportKind};

//being collected from.
pub struct ModuleFinder<'a> {
    /// Module's stored bytes
    // Maybe turn this into &str
    src_bytes: &'a [u8],
    settings: &'a ChrnSettings,
    /// Path origin so that errors can accurately report the path where the import was declared
    current_region: &'a SourceRegion,
    pos: usize,
    start: usize,
    end: usize,
}

impl ModuleFinder<'_> {
    pub fn new<'a>(
        //TODO: Need to store beg
        src_bytes: &'a [u8],
        settings: &'a ChrnSettings,
        current_region: &'a SourceRegion,
        script_start: usize,
        serial_start: Option<usize>,
    ) -> ModuleFinder<'a> {
        ModuleFinder {
            src_bytes,
            settings,
            current_region,
            pos: script_start,
            start: script_start,
            end: serial_start.unwrap_or(src_bytes.len()),
        }
    }

    /// Returns a tuple with `Bind` and all imports found on `Ok`.
    /// Returns `ConfigLoadError` on `Err`
    pub fn collect_imports(
        &mut self,
        interner: &mut Intern,
    ) -> Result<(Option<Bind>, Vec<Import>), ConfigLoadError> {
        let mut imports: Vec<Import> = Vec::new();
        let mut bind: Option<Bind> = None;

        loop {
            self.skip_until_important();

            if self.pos >= self.end && self.peek() == b'\0' {
                break;
            }

            let ch = self.peek();

            match ch {
                // b'i' => {
                //     if self.is_import() {
                //         let import = self.parse_import(interner)?;
                //         imports.push(import);
                //     }
                // }
                // b'b' if is_after_whitespace => {
                //     if self.is_bind() {
                //         bind = Some(self.parse_bind(interner)?);
                //     }
                //     self.advance();
                // }
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
                // Should probably just work with &str directly at this point
                c if c == b'i' || c == b'b' => {
                    // The operation requires a full utf-8 check
                    if self.peek_behind_char(1).is_whitespace() {
                        if c == b'i' && self.is_import() {
                            let import = self.parse_import(interner)?;
                            imports.push(import);
                        } else if c == b'b' && self.is_bind() {
                            bind = Some(self.parse_bind(interner)?);
                        }
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
        // Boolean to track if a "\" was seen since only "/" can be used to separate
        let mut saw_backslash = false;

        while self.pos < self.src_bytes.len() {
            match self.peek() {
                b'"' => {
                    self.advance();
                    break;
                }
                b'\\' => {
                    saw_backslash = true;
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }

        // - 1 to include quotes since that happens in the lexer. No other reason.
        let end = self.pos - 1;

        let path_span = SourceSpan::new(
            self.current_region.region_id,
            (start - 1) as u32,
            end as u32,
        );

        if saw_backslash {
            let core_msg = "Only '/' can be used as path separators.".to_string();

            // Since we know it's invalid

            let src_diag = SourceDiagnostic::builder(
                DiagnosticLevel::Error,
                core_msg,
                self.current_region.path_id,
            )
            .add_annotation(path_span, AnnotationKind::Primary, None)
            .build();

            return Err(ConfigLoadError::Module(src_diag));
        }

        let path_buf = self.create_pathbuf(&self.src_bytes[start..end])?;

        let import_path = match path_buf.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                let core_msg =
                    core_error::form_string_from_io_err(&e, &path_buf).unwrap_or(e.to_string());

                let src_diag = SourceDiagnostic::builder(
                    DiagnosticLevel::Error,
                    core_msg,
                    self.current_region.path_id,
                )
                .add_annotation(path_span, AnnotationKind::Primary, None)
                .build();

                return Err(ConfigLoadError::Module(src_diag));
            }
        };

        // We could check for an alias here too in case the file name is invalid and needs an alias
        //TODO:
        let file_name = match import_path.file_prefix().map(|n| n.to_str()) {
            Some(Some(n)) => n,
            _ => {
                let core_msg = format!(
                    "Failed to extract file name for path \"{}\"",
                    import_path.display()
                );

                let src_diag = SourceDiagnostic::builder(
                    DiagnosticLevel::Error,
                    core_msg,
                    self.current_region.path_id,
                )
                .add_annotation(path_span, AnnotationKind::Primary, None)
                .build();

                return Err(ConfigLoadError::General(src_diag));
            }
        };

        let name_id = interner.intern(&file_name);
        let path_id = interner.intern_path(&import_path);

        let alias_id: Option<InternedId> = if self.is_as() {
            self.skip_whitespace();
            Some(self.read_id(interner))
        } else {
            None
        };

        let import_kind = ImportKind::Source(path_id, path_span);

        Ok(Import::new(name_id, import_kind, alias_id))
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
        } else if cfg!(windows) {
            // NOTE: This may be done differently but remains a basic utf-8 check for now
            // #[cfg(windows)]
            // {
            //     use std::os::windows::ffi::OsStrExt;
            //     let os_str = OsStr::from_wide(slice);
            //
            //     return Ok(PathBuf::from(slice));
            // }
            match str::from_utf8(slice) {
                // To my knowledge, a valid UTF-8 string cannot fail conversion to a path,
                // therefore this is infailable as said by the return type, which fits whatever
                // type utilized with the From<T> conversion.
                Ok(s) => return Ok(PathBuf::from_str(&s).expect("Infailable")),
                Err(_) => {
                    let msg = "Invalid UTF-8 found within file".to_string();
                    let src_diag = SourceDiagnostic::builder(
                        DiagnosticLevel::Error,
                        msg,
                        self.current_region.path_id,
                    )
                    .build();

                    return Err(ConfigLoadError::General(src_diag));
                }
            }
        }

        match str::from_utf8(slice) {
            Ok(s) => Ok(PathBuf::from_str(&s).expect("Unwrapped twice")),
            Err(_) => {
                let msg = "Invalid UTF-8 found within file".to_string();
                let src_diag = SourceDiagnostic::builder(
                    DiagnosticLevel::Error,
                    msg,
                    self.current_region.path_id,
                )
                .build();

                return Err(ConfigLoadError::General(src_diag));
            }
        }
    }

    fn parse_bind(&mut self, interner: &mut Intern) -> Result<Bind, ConfigLoadError> {
        // skipping "
        self.advance();
        let start = self.pos;

        let mut saw_backslash = false;

        while self.pos < self.src_bytes.len() {
            match self.peek() {
                b'"' => {
                    self.advance();
                    break;
                }
                b'\\' => {
                    saw_backslash = true;
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }

        let end = self.pos - 1;
        let path_span = SourceSpan::new(
            self.current_region.region_id,
            (start - 1) as u32,
            end as u32,
        );

        if saw_backslash {
            let core_msg = "Only '/' can be used as path separators.".to_string();

            let src_diag = SourceDiagnostic::builder(
                DiagnosticLevel::Error,
                core_msg,
                self.current_region.path_id,
            )
            .add_annotation(path_span, AnnotationKind::Primary, None)
            .build();

            return Err(ConfigLoadError::Module(src_diag));
        }

        // Um uh
        let path_buf = self.create_pathbuf(&self.src_bytes[start..end]).unwrap();

        //WARN: WRONG PATH NAME
        // Please..
        let bind_path = match path_buf.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                let path = interner.search_path(self.current_region.path_id);
                // todo!("Correct the name with it's region's path name");
                let core_msg =
                    core_error::form_string_from_io_err(&e, path).unwrap_or(e.to_string());

                let src_diag = SourceDiagnostic::builder(
                    DiagnosticLevel::Error,
                    core_msg,
                    self.current_region.path_id,
                )
                .add_annotation(path_span, AnnotationKind::Primary, None)
                .build();

                return Err(ConfigLoadError::Module(src_diag));
            }
        };

        let bind_path_id = interner.intern_path(&bind_path);

        Ok(Bind::new(bind_path_id, path_span))
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

        interner.intern(&id_str)
    }

    fn peek_behind_char(&mut self, dest: usize) -> char {
        // Inclusive since otherwise it would skip the current character and there would need to be
        // a saturating sub to make up for it
        let chunk = &self.src_bytes[self.start..=self.pos];

        std::str::from_utf8(chunk)
            .ok()
            .and_then(|c| c.chars().rev().skip(dest).next())
            .unwrap_or('\0')
    }

    fn peek_behind(&mut self, dest: usize) -> u8 {
        self.src_bytes
            .get(self.pos - dest)
            .copied()
            .unwrap_or(b'\0')
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
