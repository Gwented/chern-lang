// TEST SHOULD COVER THESE DIAGNOSTICS NOT DESTROYING THINGS
/// This module represents the stage of `chrn` processing where there it may read an entire file, or
/// it may read between `@def` and `@end`. This exists so that if there is serial data within the
/// file, the entire file isn't forced to be loaded into memory, which would be a net negative.
use std::io::{BufRead, BufReader, Read};

use chrn_utils::{
    chrn_settings::ChrnSettings,
    core_error::ConfigLoadError,
    id_types::{PathId, SourceRegionId},
    intern::Intern,
    source_map::{
        source_diagnostic::{DiagnosticLevel, SourceDiagnostic, annotations::AnnotationKind},
        source_region::SourceRegion,
        source_span::SourceSpan,
    },
};

use crate::keywords::ANNOTATION_CLAUSE_SIZE;

/// 8KB read limit before aborting looking for @end in script file
const READ_LIMIT: usize = 8192;

pub struct ConfigLoader<'a, R: Read> {
    // Since region ids are not used by the loader for diagnostics, this is safe. Only the path id
    // is used.
    current_region_id: SourceRegionId,
    // Configuration file path
    current_path_id: PathId,
    handle: BufReader<R>,
    settings: &'a ChrnSettings,
    // TODO: Remove this?
    interner: &'a Intern,
    pos: usize,
}

// Gorf
/// This also exists as a local output return, which may or may not hold an error that is local to
/// this stage since the caller cares about and uses the region, but the external caller
/// (like a CLI) only wants the specific `ConfigLoaderError` that may exist.
#[derive(Debug)]
pub enum ConfigLoaderOutput {
    // Should this just be Option ConfigLoaderError?
    /// Loaded region with no issues
    Success(SourceRegion),
    /// A region that experienced an error during the loading process.
    ///
    /// This is not called "partial" because an error like `@def` without `@end` isn't a partial
    /// viewing, it's just a broken viewing.
    Broken(SourceRegion, ConfigLoadError),
    /// A region that at some point got an error to where
    /// Loading failed too early so no part of the region was read.
    UnrecoverableErr(ConfigLoadError),
    // If err != nil { return err }
}

//NOTE: This forces paths to be given, but if the chern file itself doesn't have a path given
//then the language doesn't work anyways. May leave as is.
impl<R: Read> ConfigLoader<'_, R> {
    /// Uses `PathId` for error location reporting purposes
    pub fn new<'a>(
        current_region_id: SourceRegionId,
        handle: R,
        current_path_id: PathId,
        settings: &'a ChrnSettings,
        interner: &'a Intern,
    ) -> ConfigLoader<'a, R> {
        ConfigLoader {
            current_region_id,
            settings,
            interner,
            current_path_id,
            handle: BufReader::new(handle),
            pos: 0,
        }
    }

    /// Returns a Success value of the bytes to Lex, the offset of where to start lexing if an
    /// `@def` is present, and the offset of where to start reading the serialized data if an
    /// `@def` and `@end` is present. Returns a `ConfigLoadError` upon failure that has internal
    /// error details.
    pub fn load_config(&mut self) -> ConfigLoaderOutput {
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

        // The only unrecoverable region error since it failed before and reads
        match self.handle.fill_buf() {
            Ok(_) => (),
            Err(err) => {
                return ConfigLoaderOutput::UnrecoverableErr(ConfigLoadError::IO(err));
            }
        };

        while let Some(b) = self.peek() {
            //TODO: New error enum would need to exist which specifically needs to say whether
            //or not the program should keep going.
            // Yes, I am Mythos for partaking in a form of security detection, that is the condition.
            //
            // This should also not be terminal
            if self.pos > READ_LIMIT {
                let script_type = if requires_end { "block" } else { "file" };

                panic!(
                    "Exceeded read limit `{READ_LIMIT}` while attempting to read script {script_type}"
                );
            }

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
                    let quote_type = self.advance().expect("Confirmed by match arm");

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
                            AnnotationKind::Secondary,
                            "Possible start of unclosed quote".to_string().into(),
                        );

                        if double_quotes_seen > 1 {
                            let note = "There are other quotes within the file so the line given could be incorrect".to_string();
                            diag_builder = diag_builder.add_note(note);
                        };

                        let src_diag = diag_builder.build();
                        //TEMP: Exclusive spanning will be used here :(
                        self.pos -= 1;
                        let broken_region = self.create_region(script_start, None);

                        return ConfigLoaderOutput::Broken(
                            broken_region,
                            ConfigLoadError::Diagnostic(src_diag),
                        );
                    }

                    if double_quotes_seen == 1 {
                        first_double_quote = Some(quote_start)
                    };
                }
                b'\'' => {
                    let quote_start = self.pos;
                    let quote_type = self.advance().expect("Confirmed by match arm");

                    single_quotes_seen += 1;

                    // Is there a reason for lines_read to be printed if there are multiple quotes?
                    // When are there ever NOT multiple quotes if it's in a serialized file?
                    if self.read_quotes(quote_type).is_err() {
                        let core_msg = format!("Found unclosed quotes which reached <eof>");

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
                            AnnotationKind::Secondary,
                            "Possible start of unclosed quote".to_string().into(),
                        );

                        if single_quotes_seen > 1 {
                            let note = "There are other quotes within the file so the line given could be incorrect".to_string();
                            diag_builder = diag_builder.add_note(note);
                        };

                        let src_diag = diag_builder.build();

                        let broken_region = self.create_region(script_start, None);

                        return ConfigLoaderOutput::Broken(
                            broken_region,
                            ConfigLoadError::Diagnostic(src_diag),
                        );
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
                        match self.handle_multi_comment() {
                            Ok(_) => (),
                            // Can only return err on unexpected <eof>
                            Err(cfg_err) => return ConfigLoaderOutput::UnrecoverableErr(cfg_err),
                        };
                    }
                }
                b'@' => {
                    // This @ is not skipped for the sake of keeping self.pos at the same starting point.
                    // If we are at @def, we need to read (inclusive, inclusive)
                    // where the second inclusive adds ANNOTATION_CLAUSE_SIZE to itself

                    // dbg!(self.handle.buffer()[self.pos + ANNOTATION_CLAUSE_SIZE - 1] as char);
                    // Helper boolean

                    // let buffer = &self.handle.buffer()[self.pos..];
                    // let s = str::from_utf8(buffer);
                    // WARN: Suspicious
                    //
                    // Possible source of indexing bug could be that ANNOTATION_CLAUSE_SIZE, which
                    // is sliced exclusively here, would exceed the length since it didn't ensure
                    // it's length was inside the constraints of the buffer.
                    //
                    // This is hard to track since the overwhelming majority of the time, there's at
                    // least a '\n' after @def and @end, making most operations succeed anyways
                    // since it's impossible to get the error unless the environment specifically
                    // does not have at most one extra byte
                    let can_check =
                        // DID THE - 1 FIX?
                        if self.pos + (ANNOTATION_CLAUSE_SIZE - 1) < self.handle.buffer().len() {
                            true
                        } else {
                            false
                        };

                    // If `@def` was seen, there is enough space to check, and `@end` aligns with
                    // the bytes seen, set the serial start to the position 1 after `@end`
                    if requires_end
                        && can_check
                        && &self.handle.buffer()[self.pos..self.pos + ANNOTATION_CLAUSE_SIZE]
                            == b"@end"
                    {
                        // Starts exactly 1 byte after "@end"
                        // This variable being set is proof there was a script block, but not proof
                        // there's serial data
                        let serial_start = self.pos + ANNOTATION_CLAUSE_SIZE;

                        //WARN: Doesn't use helper because the helper doesn't take in a position
                        //argument. It doesn't take one because it doesn't seem meaningful, it seems
                        //like a forced compatibility later for this one singular case.
                        let region = SourceRegion::new(
                            self.handle.buffer()[..self.pos + ANNOTATION_CLAUSE_SIZE].to_vec(),
                            self.current_region_id,
                            self.current_path_id,
                            script_start,
                            Some(serial_start),
                        );

                        return ConfigLoaderOutput::Success(region);
                    }

                    // If no @def has been seen yet, there is enough space to check, and the next 4
                    // bytes are "@def", set a requirement for an "@end", set the `script_start` to
                    // the "@" in "@def", skip "@def", then set the Option def_span to the current
                    // position span
                    if !requires_end
                        && can_check
                        // Since we are at "@" if we want to read "@def"/"@end" it's an
                        // (inclusive, exclusive) spanning operation since "@def".len() + 4 would be
                        // 1 above the actual length
                        && &self.handle.buffer()[self.pos..self.pos + ANNOTATION_CLAUSE_SIZE]
                            == b"@def"
                    {
                        requires_end = true;
                        script_start = self.pos;

                        // WARN:
                        // Stops at "f" because there may not be a byte after f, which should be
                        // handled by the main loop.
                        //
                        // Skips "@def" to the byte after it. This is safe since it will eithe
                        // return '\0' or `None` which both avoid over-indexing being a possibility
                        self.skip(ANNOTATION_CLAUSE_SIZE);

                        def_span = Some(SourceSpan::new(
                            self.current_region_id,
                            span_start as u32,
                            // Needs - 1 since it stops one after the f in def
                            (self.pos - 1) as u32,
                        ));

                        // Needs to continue or the last advance causes one-off errors since it
                        // conflicts with @def which already deals with itself
                        continue;
                    }

                    // This should not fail since if `@def` and `@end` fail, it should not consume
                    // anything in the process and just treat this as though it were a singular @ to
                    // skip
                    debug_assert_eq!(self.handle.buffer()[self.pos], b'@');
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }
        // TODO: Assert this...

        let region = self.create_region(script_start, None);

        // Case of no @def and no @end which requires a '\0' return since the entire file should be
        // read. This does not mean it is correct, it only means the read limit wasn't reached.
        if !requires_end {
            ConfigLoaderOutput::Success(region)
        } else {
            // Case of end <eof> being reached, which means it is within `READ_LIMIT` since it
            // didn't actively reach it
            let core_msg = format!("Could not find `@end` after `@def`");

            let def_span = def_span.expect("@def must exist for this branch to be seen");
            // Explicitly declaring this so the end of the file can be pointed at too

            // Ensuring an indexable character is the last character
            //WARN: Need to make sure this doesn't break things
            // WARN: ENSURE THIS DOES NOT BREAK ANYTHING

            // This - 1 has the same reason as many times in the lexer, where since spans are
            // (inclusive, inclusive), when we use advance, and then break, that means we are
            // technically the length of the point of interest + 1, but since we need an inclusive
            // end it needs to - 1 itself.
            //
            // So, if we have "text\0", it advances "t" stopping at "\0", then it breaks, but since it's inclusive,
            // that would mean it's over-indexing the actual span, so we need to - 1 to go back to
            // the last "t" in "text"
            //
            //
            // TODO: Change everything to exclusive.
            let eof_pos = (self.pos - 1) as u32;

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

            ConfigLoaderOutput::Broken(region, ConfigLoadError::Diagnostic(src_diag))
        }
    }

    /// Returns a result instead of an option because if there are unclosed quotes and this method
    /// fails, it would need return a Some value which DOESN'T represent a failure, making it
    /// misleading.
    ///
    //// Returns `true` on success, `false` on failure (Maybe?)
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

    /// Helper for reducing region creation boiler-plate
    fn create_region(&self, script_start: usize, serial_start_opt: Option<usize>) -> SourceRegion {
        SourceRegion::new(
            self.handle.buffer()[..self.pos].to_vec(),
            self.current_region_id,
            self.current_path_id,
            script_start,
            serial_start_opt,
        )
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

            return Err(ConfigLoadError::Diagnostic(src_diag));
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
