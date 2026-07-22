// Has weird behavior and silent errors when it comes to import keyword usage
// FIX: When an import is next to something like, "@defimport" it is NOT identified because the i
use std::{ffi::OsStr, path::PathBuf, str::FromStr};

use chrn_utils::chrn_config::ChrnConfig;
use chrn_utils::source_map::source_diagnostic::DiagnosticLevel;
use chrn_utils::source_map::source_diagnostic::annotations::AnnotationKind;
use chrn_utils::{
    core_error::{self},
    id_types::{InternedId, ModuleId, PathId},
    intern::Intern,
    source_map::{
        source_diagnostic::SourceDiagnostic, source_region::SourceRegion, source_span::SourceSpan,
    },
};

use crate::modules::{Bind, Import, ImportKind};

/// Module graph start-up structure that finds imports and assigns them a `ModuleId` from `seen`.
///
/// This is not recursive in any way, it's just a mini parser which uses the most minimal syntax
/// possible to search `src_bytes` and identifiy imports and bind usage where possible.
pub struct ModuleFinder<'a> {
    /// Current module's bytes
    // Maybe turn this into &str
    src_bytes: &'a [u8],
    cfg: &'a ChrnConfig,
    seen: &'a mut Vec<(PathId, ModuleId)>,
    diags: Vec<SourceDiagnostic>,
    /// Path origin so that errors can accurately report the path where the import was declared
    current_region: &'a SourceRegion,
    pos: usize,
    script_start: usize,
    // NOT NEEDED BUT STAYING IN CASE
    serial_start: usize,
}

impl ModuleFinder<'_> {
    pub fn new<'a>(
        src_bytes: &'a [u8],
        cfg: &'a ChrnConfig,
        seen: &'a mut Vec<(PathId, ModuleId)>,
        current_region: &'a SourceRegion,
        script_start: usize,
        serial_start: Option<usize>,
    ) -> ModuleFinder<'a> {
        ModuleFinder {
            src_bytes,
            cfg,
            current_region,
            diags: Vec::new(),
            seen,
            pos: 0,
            script_start,
            // If there is no serial start then it's a script file not a script block with @def ->
            // @end
            serial_start: serial_start.unwrap_or(src_bytes.len()),
        }
    }

    /// Returns a tuple with `Bind` and all imports found on `Ok`.
    /// Returns `ConfigLoadError` on `Err`
    pub fn collect_imports(
        &mut self,
        interner: &mut Intern,
        // A little crowded..
    ) -> (Option<Bind>, Vec<Import>, Vec<SourceDiagnostic>) {
        let mut imports: Vec<Import> = Vec::new();
        let mut bind: Option<Bind> = None;

        loop {
            self.skip_until_important();

            if self.pos >= self.src_bytes.len() && self.peek() == b'\0' {
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
                            let import = match self.parse_import(interner) {
                                Ok(i) => i,
                                Err(d) => {
                                    self.diags.push(d);
                                    continue;
                                }
                            };

                            imports.push(import);
                        } else if c == b'b' && self.is_bind() {
                            bind = match self.parse_bind(interner) {
                                Ok(b) => Some(b),
                                Err(d) => {
                                    self.diags.push(d);
                                    continue;
                                }
                            };
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

        let mut diags = Vec::new();
        diags.append(&mut self.diags);

        (bind, imports, diags)
    }

    /// Assumes the starting point is at the start quote
    fn parse_import(&mut self, interner: &mut Intern) -> Result<Import, SourceDiagnostic> {
        self.advance();
        let start_cursor = self.pos;
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

        // - 1 for same reason as in lexer. Before breaking the last quote is skipped, and since
        // span ends are exclusive, the end pos would be one after the end quote, so we need to go
        // back 1 to properly sit at the end quote
        let end_cursor = self.pos - 1;

        let path_span = SourceSpan::new(
            self.current_region.region_id,
            // To include start quote
            start_cursor as u32,
            // To include end quote
            end_cursor as u32,
        );

        if saw_backslash {
            let core_msg = "Only '/' can be used as path separators.".to_string();

            // Since we know it's invalid

            //NOTE: Maybe an error code?
            let src_diag = SourceDiagnostic::builder(
                None,
                DiagnosticLevel::Error,
                core_msg,
                self.current_region.path_id,
            )
            .add_annotation(path_span, AnnotationKind::Primary, None)
            .build();

            return Err(src_diag);
        }

        let path_buf = self.create_pathbuf(&self.src_bytes[start_cursor..end_cursor])?;

        let import_path = match path_buf.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                let core_msg =
                    core_error::form_string_from_io_err(&e, &path_buf).unwrap_or(e.to_string());

                let src_diag = SourceDiagnostic::builder(
                    None,
                    DiagnosticLevel::Error,
                    core_msg,
                    self.current_region.path_id,
                )
                .add_annotation(path_span, AnnotationKind::Primary, None)
                .build();

                return Err(src_diag);
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

                //TODO: Aliasing
                let src_diag = SourceDiagnostic::builder(
                    None,
                    DiagnosticLevel::Error,
                    core_msg,
                    self.current_region.path_id,
                )
                .add_annotation(path_span, AnnotationKind::Primary, None)
                .build();

                return Err(src_diag);
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

        let mod_id =
            // If there exists a module attached to the path seen, the import being viewed has
            // already been processed and should maintain the same module id
            if let Some((_, inner_mod_id)) = self.seen.iter().find(|(p_id, _)| *p_id == path_id) {
                *inner_mod_id
            } else {
            // First time seeing this path, so a new key = PathId, Value = ModuleId relationship is
            // made
                let new_mod_id = ModuleId::new(self.seen.len() as u32);
                self.seen.push((path_id, new_mod_id));
                new_mod_id
            };
        let import = Import::new(name_id, mod_id, import_kind, alias_id);

        self.cfg.logger().log_dbg(|| {
            let import_name = interner.search(name_id);
            let region_path = interner.search_path(self.current_region.path_id);
            format!(
                "Created import `{import_name}` (region=\"{}\")",
                region_path.display()
            )
        });

        Ok(import)
    }

    //WARN: Will be placed in different module
    fn create_pathbuf(&self, slice: &[u8]) -> Result<PathBuf, SourceDiagnostic> {
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
                // therefore this is infallable as said by the return type, which fits whatever
                // type utilized with the From<T> conversion.
                Ok(s) => return Ok(PathBuf::from_str(&s).expect("Infallable")),
                Err(_) => {
                    let msg = "Invalid UTF-8 found within file".to_string();
                    let src_diag = SourceDiagnostic::builder(
                        None,
                        DiagnosticLevel::Error,
                        msg,
                        self.current_region.path_id,
                    )
                    .build();

                    return Err(src_diag);
                }
            }
        }

        match str::from_utf8(slice) {
            Ok(s) => Ok(PathBuf::from_str(&s).expect("Infallible")),
            Err(_) => {
                let msg = "Invalid UTF-8 found within file".to_string();
                let src_diag = SourceDiagnostic::builder(
                    None,
                    DiagnosticLevel::Error,
                    msg,
                    self.current_region.path_id,
                )
                .build();

                Err(src_diag)
            }
        }
    }

    fn parse_bind(&mut self, interner: &mut Intern) -> Result<Bind, SourceDiagnostic> {
        // skipping "
        self.advance();
        let start_cursor = self.pos;

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

        let end_cursor = self.pos - 1;

        let path_span = SourceSpan::new(
            self.current_region.region_id,
            start_cursor as u32,
            end_cursor as u32,
        );

        if saw_backslash {
            let core_msg = "Only '/' can be used as path separators.".to_string();

            //NOTE: Maybe
            let src_diag = SourceDiagnostic::builder(
                None,
                DiagnosticLevel::Error,
                core_msg,
                self.current_region.path_id,
            )
            .add_annotation(path_span, AnnotationKind::Primary, None)
            .build();

            return Err(src_diag);
        }

        // WHY WAS THIS UNWRAP FOR SO LONG
        let path_buf = self.create_pathbuf(&self.src_bytes[start_cursor..end_cursor])?;

        //WARN: WRONG PATH NAME
        // Please..
        let bind_path = match path_buf.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                let core_msg =
                    core_error::form_string_from_io_err(&e, &path_buf).unwrap_or(e.to_string());

                let src_diag = SourceDiagnostic::builder(
                    None,
                    DiagnosticLevel::Error,
                    core_msg,
                    self.current_region.path_id,
                )
                .add_annotation(path_span, AnnotationKind::Primary, None)
                .build();

                return Err(src_diag);
            }
        };

        let bind_path_id = interner.intern_path(&bind_path);
        let bind = Bind::new(bind_path_id, path_span);

        self.cfg.logger().log_dbg(|| {
            let bind_path = interner.search_path(bind_path_id);
            let region_path = interner.search_path(self.current_region.path_id);
            format!(
                "Created bind which points to `{}` for region \"{}\"",
                bind_path.display(),
                region_path.display()
            )
        });

        Ok(bind)
    }

    fn read_id(&mut self, interner: &mut Intern) -> InternedId {
        let start = self.pos;

        while (self.pos < self.src_bytes.len() && self.peek_char().is_alphanumeric())
            || (self.pos < self.src_bytes.len() && self.peek() == b'_')
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
    //FIX:

    fn peek_behind_char(&mut self, dest: usize) -> char {
        // Inclusive since otherwise it would skip the current character and there would need to be
        // a saturating sub to make up for it
        let chunk = &self.src_bytes[0..=self.pos];

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
        while self.pos < self.src_bytes.len() && self.peek() != b'\n' {
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
        while self.pos <= self.src_bytes.len()
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
