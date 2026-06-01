use std::io::{BufRead, BufReader, Read};

use crate::{
    chrn_settings::ChrnSettings,
    core_error::ConfigLoadError,
    id_types::{PathId, SourceRegionId},
    intern::Intern,
    keywords::DEFINITION_SIZE,
    source_map::{
        source_diagnostic::{AnnotationKind, DiagnosticLevel, SourceDiagnostic},
        source_region::SourceRegion,
        source_span::SourceSpan,
    },
};

const READ_LIMIT_OFFSET: usize = 500;

pub struct ChrnConfigLoader<'a, R: Read> {
    // Configuration file path
    current_region_id: SourceRegionId,
    current_path_id: PathId,
    handle: BufReader<R>,
    settings: &'a ChrnSettings,
    interner: &'a Intern,
    pos: usize,
}

//NOTE: This forces paths to be given, but if the chern file itself doesn't have a path given
//then the language doesn't work anyways. May leave as is.
impl<R: Read> ChrnConfigLoader<'_, R> {
    /// Uses `PathId` for error location reporting purposes
    pub fn new<'a>(
        current_region_id: SourceRegionId,
        handle: R,
        path_id: PathId,
        settings: &'a ChrnSettings,
        interner: &'a Intern,
    ) -> ChrnConfigLoader<'a, R> {
        ChrnConfigLoader {
            current_region_id,
            settings,
            interner,
            current_path_id: path_id,
            handle: BufReader::new(handle),
            pos: 0,
        }
    }

    /// Returns a Success value of the bytes to Lex, the offset of where to start lexing if an
    /// `@def` is present, and the offset of where to start reading the serialized data if an
    /// `@def` and `@end` is present. Returns a `ConfigLoadError` upon failure that has internal
    /// error details.
    pub fn load_config(&mut self) -> Result<SourceRegion, ConfigLoadError> {
        // Doesn't NEED definition but will error if declared and not closed
        let mut requires_end = false;

        // Setup to allow for unclosed quotes to be tracked since this stage is very sensitive to
        // such errors.
        let mut first_double_quote = None;
        let mut first_single_quote = None;

        let mut double_quotes_seen = 0;
        let mut single_quotes_seen = 0;

        let mut script_start = 0;

        let mut def_span: Option<SourceSpan> = None;

        self.handle.fill_buf()?;

        while let Some(b) = self.peek() {
            if b == b'\0' {
                break;
            }

            let span_start = self.pos;

            match b {
                // May reduce duplication by using mini-state that keeps track of quotes but fine
                // for now
                b'"' => {
                    let quote_start = self.pos;
                    // Even though this can't fail
                    // Reward hacking
                    let quote_type = self.advance().unwrap_or(b'\0');

                    double_quotes_seen += 1;

                    // Is there a reason for lines_read to be printed if there are multiple quotes?
                    // When are there ever NOT multiple quotes if it's in a serialized file?
                    if self.read_quotes(quote_type).is_err() {
                        let core_msg = "Found unclosed quotes which reached <eof>".to_string();

                        let quote_start = quote_start as u32;
                        let q_span =
                            SourceSpan::new(self.current_region_id, quote_start, quote_start);

                        let mut diag_builder = SourceDiagnostic::builder(
                            DiagnosticLevel::Error,
                            core_msg,
                            self.current_path_id,
                        )
                        .add_annotation(
                            q_span,
                            AnnotationKind::Primary,
                            "Possible unclosed quotes".to_string().into(),
                        );

                        if double_quotes_seen > 1 {
                            let note = "There are other quotes within the file so the line given could be incorrect".to_string();
                            diag_builder = diag_builder.add_note(note);
                        };

                        let src_diag = diag_builder.build();

                        return Err(ConfigLoadError::General(src_diag));
                    }

                    if double_quotes_seen == 1 {
                        first_double_quote = Some(quote_start)
                    };
                }
                b'\'' => {
                    let quote_start = self.pos;
                    let quote_type = self.advance().unwrap_or(b'\0');

                    single_quotes_seen += 1;

                    // Is there a reason for lines_read to be printed if there are multiple quotes?
                    // When are there ever NOT multiple quotes if it's in a serialized file?
                    if self.read_quotes(quote_type).is_err() {
                        let note = if single_quotes_seen > 1 {
                            "\nnote: There are other quotes within the file so the line given could be incorrect"
                        } else {
                            ""
                        };

                        let core_msg = format!("Found unclosed quotes which reached <eof>{}", note);

                        let q_span = SourceSpan::new(
                            self.current_region_id,
                            quote_start as u32,
                            quote_start as u32,
                        );

                        let mut diag_builder = SourceDiagnostic::builder(
                            DiagnosticLevel::Error,
                            core_msg,
                            self.current_path_id,
                        )
                        .add_annotation(
                            q_span,
                            AnnotationKind::Primary,
                            "Possible unclosed quotes".to_string().into(),
                        );

                        if single_quotes_seen > 1 {
                            let note = "There are other quotes within the file so the line given could be incorrect".to_string();
                            diag_builder = diag_builder.add_note(note);
                        };

                        let src_diag = diag_builder.build();

                        return Err(ConfigLoadError::General(src_diag));
                    }

                    if single_quotes_seen == 1 {
                        first_single_quote = Some(quote_start)
                    };
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
                    // Is less than or equal to because the actual slice is start..end so
                    // self.pos could be equal to buffer.len()
                    let can_check = if self.pos + DEFINITION_SIZE <= self.handle.buffer().len() {
                        true
                    } else {
                        false
                    };

                    // If there was an '@def' spotted but there's nothing enough space in the
                    // file to check
                    if requires_end && !can_check {
                        break;
                    } else if requires_end
                        && &self.handle.buffer()[self.pos..self.pos + DEFINITION_SIZE] == b"@end"
                    {
                        let serial_start = self.pos + DEFINITION_SIZE;

                        let region = SourceRegion::new(
                            self.handle.buffer()[..self.pos + DEFINITION_SIZE].to_vec(),
                            self.current_region_id,
                            self.current_path_id,
                            script_start,
                            Some(serial_start),
                        );

                        return Ok(region);
                    }

                    if !requires_end && !can_check {
                        break;
                    } else if !requires_end
                        && &self.handle.buffer()[self.pos..self.pos + DEFINITION_SIZE] == b"@def"
                    {
                        requires_end = true;
                        script_start = self.pos;
                        self.skip(DEFINITION_SIZE);
                        // self.pos + DEFINITION_SIZE stops exactly at the 'f' in '@def' which
                        // doesn't align with (inclusive, exclusive) spanning within the lexer so
                        // it needs to be taken down by 1.
                        def_span = Some(SourceSpan::new(
                            self.current_region_id,
                            span_start as u32,
                            (self.pos - 1) as u32,
                        ));
                    }

                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }
        // TODO: Assert this...

        // Case of no @def and no @end which requires a '0' return since the entire file should be
        // read. This does not mean it is correct, it only means the read limit wasn't reached.
        if !requires_end {
            let region = SourceRegion::new(
                self.handle.buffer()[..self.pos].to_vec(),
                self.current_region_id,
                self.current_path_id,
                script_start,
                None,
            );

            Ok(region)
        } else {
            let core_msg = format!("Could not find `@end` after `@def`");

            let def_span = def_span.expect("@def must exist for this branch to be seen");
            // Explicitly declaring this so the end of the file can be pointed at too

            // Ensuring an indexable character is the last character
            // FIX: This is a flaw of the reporter not spanning over EOF
            let eof_pos = if self.pos == self.handle.buffer().len() {
                self.pos - 1
            } else {
                self.pos
            } as u32;

            let eof_span = SourceSpan::new(self.current_region_id, eof_pos, eof_pos);

            let src_diag =
                SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, self.current_path_id)
                    .add_annotation(
                        def_span,
                        AnnotationKind::Secondary,
                        "`@def` started here".to_string().into(),
                    )
                    .add_annotation(
                        eof_span,
                        AnnotationKind::Primary,
                        "Unexpected <eof>".to_string().into(),
                    )
                    .build();

            Err(ConfigLoadError::General(src_diag))
        }
    }

    /// Returns a result instead of an option because if there are unclosed quotes and this method
    /// fails, it would need return a Some value which DOESN'T represent a failure, making it
    /// misleading.
    // TODO: LEXER SHOULD ALSO HANDLE THIS ALONE
    fn read_quotes(&mut self, quote_type: u8) -> Result<(), ()> {
        // let mut read_bytes = 0;

        while let Some(b) = self.peek() {
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

    fn handle_multi_comment(&mut self) -> Result<(), ConfigLoadError> {
        let mut depth = 1;

        // To adjust multi-comment start to the first '/'. /*c - 2 = /
        let comment_start = self.pos - 2;

        while let Some(current_byte) = self.peek()
            && depth > 0
        {
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
            let core_msg = format!("Found unclosed multi-line comment in script");

            // To include full multi-line syntax. / + 1 = /*
            let comment_start = comment_start as u32;
            let comment_start_span =
                SourceSpan::new(self.current_region_id, comment_start, comment_start + 1);

            let current_pos = self.pos as u32;
            let eof_span = SourceSpan::new(self.current_region_id, current_pos, current_pos);

            let src_diag =
                SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, self.current_path_id)
                    .add_annotation(
                        comment_start_span,
                        AnnotationKind::Secondary,
                        "Comment starts here".to_string().into(),
                    )
                    .add_annotation(
                        eof_span,
                        AnnotationKind::Primary,
                        "Unexpected <eof>".to_string().into(),
                    )
                    .build();

            return Err(ConfigLoadError::General(src_diag));
        }

        Ok(())
    }

    fn skip(&mut self, dest: usize) {
        self.pos += dest;
    }

    fn advance(&mut self) -> Option<u8> {
        let b = self.peek();

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
