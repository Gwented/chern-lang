//FIXME: FIX REQUIRES END MESSAGE
use std::{
    io::{BufRead, BufReader, Read},
    path::Path,
};

use chern_core::{keywords::DEFINITION_SIZE, quote_model};
use common::{chern_settings::ChernSettings, core_error::ConfigLoadError, reporter, span::Span};

use crate::modules::ModuleMetadata;

const READ_LIMIT_OFFSET: usize = 500;

// More inclusive name
pub struct ChernConfigLoader<'a, R: Read> {
    // Configuration file path
    path: &'a Path,
    handle: BufReader<R>,
    settings: &'a ChernSettings,
    pos: usize,
    lines_read: usize,
}

//NOTE: This forces paths to be given, but if the chern file itself doesn't have a path given
//then the language doesn't work anyways. May leave as is.
impl<R: Read> ChernConfigLoader<'_, R> {
    pub fn new<'a>(
        path: &'a Path,
        handle: R,
        settings: &'a ChernSettings,
    ) -> ChernConfigLoader<'a, R> {
        ChernConfigLoader {
            path,
            settings,
            handle: BufReader::new(handle),
            pos: 0,
            lines_read: 1,
        }
    }

    /// Returns a Success value of the bytes to Lex, the offset of where to start lexing if an
    /// `@def` is present, and the offset of where to start reading the serialized data if an
    /// `@def` and `@end` is present. Returns a `ConfigLoadError` upon failure that has internal
    /// error details.
    pub fn load_config(&mut self) -> Result<ModuleMetadata, ConfigLoadError> {
        // Doesn't NEED definition but will error if declared and not closed
        let mut requires_end = false;

        // Setup to allow for unclosed quotes to be tracked since this stage is very sensitive to
        // such errors.
        let mut first_double_quote = None;
        let mut first_single_quote = None;

        let mut double_quotes_seen = 0;
        let mut single_quotes_seen = 0;

        let mut lex_start = 0;

        let mut def_span: Option<Span> = None;

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
                    let quote_type = self.advance().unwrap_or(b'\0');

                    if quote_type == b'\'' {
                        single_quotes_seen += 1;
                    } else {
                        double_quotes_seen += 1;
                    };

                    // Is there a reason for lines_read to be printed if there are multiple quotes?
                    // When are there ever NOT multiple quotes if it's in a serialized file?
                    if self.read_quotes(quote_type).is_err() {
                        let note = if double_quotes_seen > 1 || single_quotes_seen > 1 {
                            "\nnote: There are other quotes within the file so the line given could be incorrect"
                        } else {
                            ""
                        };

                        let msg = format!("Found unclosed quotes which reached <eof>{}", note);

                        let spans = if double_quotes_seen > 1 {
                            let start = first_double_quote.expect("Proven to be > 1");

                            let end = if self.pos + READ_LIMIT_OFFSET < self.handle.buffer().len() {
                                self.pos + READ_LIMIT_OFFSET
                            } else {
                                self.handle.buffer().len()
                            };

                            let search_range = start..end;

                            quote_model::quote_start_probability(
                                self.handle.buffer(),
                                quote_type as char,
                                search_range,
                            )
                        } else {
                            [Span::new(quote_start, quote_start)].to_vec()
                        };

                        let ln_data = reporter::form_err_diag(
                            self.handle.buffer(),
                            &spans,
                            self.settings.can_color,
                        );

                        let err_msg = reporter::standardize_err(
                            &msg,
                            &ln_data,
                            "",
                            self.path,
                            self.settings.can_color,
                        );

                        return Err(ConfigLoadError::Unclosed(err_msg));
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

                        let msg = format!("Found unclosed quotes which reached <eof>{}", note);

                        let spans = if single_quotes_seen > 1 {
                            let start = first_single_quote.expect("Proven to be > 1");

                            let end = if self.pos + READ_LIMIT_OFFSET < self.handle.buffer().len() {
                                self.pos + READ_LIMIT_OFFSET
                            } else {
                                self.handle.buffer().len()
                            };

                            let search_range = start..end;

                            quote_model::quote_start_probability(
                                self.handle.buffer(),
                                quote_type as char,
                                search_range,
                            )
                        } else {
                            [Span::new(quote_start, quote_start)].to_vec()
                        };

                        let ln_data = reporter::form_err_diag(
                            self.handle.buffer(),
                            &spans,
                            self.settings.can_color,
                        );

                        let err_msg = reporter::standardize_err(
                            &msg,
                            &ln_data,
                            "",
                            self.path,
                            self.settings.can_color,
                        );

                        return Err(ConfigLoadError::Unclosed(err_msg));
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

                        return Ok(ModuleMetadata::new(
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
                        self.skip(DEFINITION_SIZE);
                        // self.pos + DEFINITION_SIZE stops exactly at the 'f' in '@def' which
                        // doesn't align with (inclusive, exclusive) spanning within the lexer so
                        // it needs to be taken down by 1.
                        def_span = Some(Span::new(span_start, self.pos - 1));
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
            Ok(ModuleMetadata::new(
                self.handle.buffer()[..self.pos].to_vec(),
                lex_start,
                None,
            ))
        } else {
            let msg = format!("Could not find `@end` after `@def`");

            let def_span = def_span.expect("@def must exist for this branch to be seen");
            // Explicitly declaring this so the end of the file can be pointed at too

            // Ensuring an indexable character is the last character
            let eof_pos = if self.pos == self.handle.buffer().len() {
                self.pos - 1
            } else {
                self.pos
            };

            let eof_span = Span::new(eof_pos, eof_pos);

            let ln_data = reporter::form_err_diag(
                self.handle.buffer(),
                &[def_span, eof_span],
                self.settings.can_color,
            );

            let err_msg =
                reporter::standardize_err(&msg, &ln_data, "", self.path, self.settings.can_color);

            Err(ConfigLoadError::Unclosed(err_msg))
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
            let msg = format!("Found unclosed multi-line comment in configuration file");

            // To include full multi-line syntax. / + 1 = /*
            let comment_span = Span::new(comment_start, comment_start + 1);

            let eof_span = Span::new(self.pos, self.pos);

            let ln_data = reporter::form_err_diag(
                self.handle.buffer(),
                &[comment_span, eof_span],
                self.settings.can_color,
            );

            let err_msg =
                reporter::standardize_err(&msg, &ln_data, "", self.path, self.settings.can_color);

            return Err(ConfigLoadError::Unclosed(err_msg));
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
