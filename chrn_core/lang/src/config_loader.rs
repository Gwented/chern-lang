// This is not getting streamed right now. See `IMPORTANT.txt`
//FIX: This should probably be in compilation
///! This module represents the stage of `chrn` processing where there it may read an entire file, or
///! it may read between `@def` and `@end`. This exists so that if there is serial data within the
///! file, the entire file isn't forced to be loaded into memory, which would be a net negative.
//TODO: Need relative spanning in renderers
use std::io::{BufRead, BufReader, Read};

use chrn_utils::{
    chrn_config::ChrnConfig,
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
/// Can read 32KB before stopping if no `@def` or EOF is found
const MAX_SEARCH_READ: usize = 1024 * 32;

/// Size of the buffered reader used here of 64KB
const BUFFER_SIZE: usize = 1024 * 64;

// Can read at most 32 KB before an @def, can read at most 32 KB after an @def

//TODO: A script start should dictate how many bytes can be read.
//So, if script_start is 2048, total_buffer is now starting at byte 2048 of handle, and it has 16KB
//from that point forward.
pub struct ConfigLoader<'a, R: Read> {
    // Since region ids are not used by the loader for diagnostics, this is safe. Only the path id
    // is used.
    current_region_id: SourceRegionId,
    // Script file/block path
    current_path_id: PathId,
    handle: BufReader<R>,
    cfg: &'a ChrnConfig,
    ln_num_tracker: NumberTracker,
    col_tracker: NumberTracker,
    // TODO: Remove this?
    interner: &'a Intern,
    /// Position specifically used for navigating the IO buffer
    cursor: usize,
    /// The max amount of bytes to stop it, which is dynamically set, hence why it's apart of the struct.
    limit: usize,
    /// Overall bytes consumed
    bytes_consumed: usize,
}

// At most will read

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
        cfg: &'a ChrnConfig,
        interner: &'a Intern,
    ) -> ConfigLoader<'a, R> {
        ConfigLoader {
            current_region_id,
            cfg,
            interner,
            current_path_id,
            handle: BufReader::with_capacity(BUFFER_SIZE, handle),
            col_tracker: NumberTracker::new(1),
            ln_num_tracker: NumberTracker::new(1),
            cursor: 0,
            limit: MAX_SEARCH_READ,
            bytes_consumed: 0,
            // IGNORE THIS I NEED TO KEEP THE TRAIN OF THOUGHT
            // persistent_buffer: vec![0u8; chrn_utils::MAX_REGION_SIZE],
            // state: SearchingState::InDef,
            // loaded_portions: vec![0u8; chrn_utils::MAX_REGION_SIZE],
            // current_turns: 0,
            // total_turns: 0,
            // stuff: [None, None, None],
            // stuff_cursor: 0,
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

        // ONLY FILL AS OF RIGHT NOW
        match self.handle.fill_buf() {
            Ok(_) => (),
            Err(err) => {
                let out = ConfigLoaderOutput::UnrecoverableErr(ConfigLoadError::IO(err));
                return out;
            }
        }

        while let Some(b) = self.peek() {
            // dbg!(
            //     self.handle.buffer().get(self.pos).map(|c| *c as char),
            //     self.pos
            // );
            // What about um...macros?
            // if self.has_def && self.cursor == 5 {
            //     dbg!(b as char);
            // }
            // dbg!(b as char);
            // dbg!(self.pos);

            // This should also not be terminal
            // if self.pos >= chrn_utils::MAX_REGION_SIZE {
            //     let script_type = if requires_end { "block" } else { "file" };
            //
            //     panic!(
            //         "Exceeded read limit `{}` while attempting to read script {script_type}",
            //         chrn_utils::MAX_REGION_SIZE
            //     );
            // }

            // This probably won't exist anymore
            // if b == b'\0' {
            //     break;
            // }

            let span_start = self.cursor;

            match b {
                // May reduce duplication by using mini-state that keeps track of quotes but fine
                // for now
                b'"' => {
                    let quote_start = self.cursor;
                    // Even though this can't fail
                    let quote_type = self.advance().expect("Confirmed by match arm");

                    double_quotes_seen += 1;

                    // Is there a reason for lines_read to be printed if there are multiple quotes?
                    // When are there ever NOT multiple quotes if it's in a serialized file?
                    if self.read_quotes(quote_type).is_err() {
                        let core_msg = "Found unclosed quotes which reached <eof>".to_string();

                        let quote_start = quote_start as u32;
                        let q_span =
                            SourceSpan::new(self.current_region_id, quote_start, quote_start + 1);

                        let mut diag_builder = SourceDiagnostic::builder(
                            DiagnosticLevel::Error,
                            core_msg,
                            self.current_path_id,
                        )
                        .add_annotation(
                            q_span,
                            AnnotationKind::Primary,
                            "Possible start of unclosed quote".to_string().into(),
                        );

                        if double_quotes_seen > 1 {
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

                    if double_quotes_seen == 1 {
                        first_double_quote = Some(quote_start)
                    };
                }
                b'\'' => {
                    let quote_start = self.cursor;
                    let quote_type = self.advance().expect("Confirmed by match arm");

                    single_quotes_seen += 1;

                    // Is there a reason for lines_read to be printed if there are multiple quotes?
                    // When are there ever NOT multiple quotes if it's in a serialized file?
                    if self.read_quotes(quote_type).is_err() {
                        let core_msg = format!("Found unclosed quotes which reached <eof>");

                        let q_span = SourceSpan::new(
                            self.current_region_id,
                            quote_start as u32,
                            quote_start as u32 + 1,
                        );

                        let mut diag_builder = SourceDiagnostic::builder(
                            DiagnosticLevel::Error,
                            core_msg,
                            self.current_path_id,
                        )
                        .add_annotation(
                            q_span,
                            AnnotationKind::Primary,
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
                        _ = self.advance();
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

                    // Helper boolean
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
                        if self.cursor + (ANNOTATION_CLAUSE_SIZE - 1) < self.handle.buffer().len() {
                            true
                        } else {
                            false
                        };

                    // OLD BEHAVIOR THAT MAY BE RE-APPLIED
                    // If `@def` was seen, there is enough space to check, and `@end` aligns with
                    // the bytes seen, set the serial start to the position 1 after `@end`
                    // OLD BEHAVIOR THAT MAY BE RE-APPLIED
                    //
                    // This no longer requires @def first for now, so files can end with @end
                    // without any start syntax.
                    //
                    // Probably should not be allowed since the behavior from this is fairly
                    // non-deterministic unless someone is actively counting bytes
                    if can_check
                        && &self.handle.buffer()[self.cursor..self.cursor + ANNOTATION_CLAUSE_SIZE]
                            == b"@end"
                    {
                        // Starts exactly 1 byte after "@end"
                        // This variable being set is proof there was a script block, but not proof
                        // there's serial data
                        let serial_start = self.cursor + ANNOTATION_CLAUSE_SIZE;
                        // dbg!(str::from_utf8(
                        //     &self.handle.buffer()[script_start..serial_start]
                        // ));
                        // panic!("Not done");

                        //WARN: Doesn't use helper because the helper doesn't take in a position
                        //argument. It doesn't take one because it doesn't seem meaningful, it seems
                        //like a forced compatibility later for this one singular case.
                        let region = SourceRegion::new(
                            self.ln_num_tracker.val(),
                            self.col_tracker.val(),
                            self.handle.buffer()[script_start..serial_start].to_vec(),
                            self.current_region_id,
                            self.current_path_id,
                            script_start,
                            Some(serial_start),
                        );
                        // dbg!(region.abs_ln_num_start, region.abs_col_start);
                        // panic!();
                        // dbg!(str::from_utf8((&region.src_bytes)));
                        // panic!();

                        return ConfigLoaderOutput::Success(region);
                    }

                    // If no @def has been seen yet, there is enough space to check, and the next 4
                    // bytes are "@def", set a requirement for an "@end", set the `script_start` to
                    // the "@" in "@def", skip "@def", then set the Option def_span to the current
                    // position span

                    // dbg!(str::from_utf8(
                    //     &self.handle.buffer()[self.pos..self.pos + ANNOTATION_CLAUSE_SIZE]
                    // ));
                    if !requires_end
                        && can_check
                        // Since we are at "@" if we want to read "@def"/"@end" it's an
                        // (inclusive, exclusive) spanning operation since "@def".len() + 4 would be
                        // 1 above the actual length
                        && &self.handle.buffer()[self.cursor..self.cursor + ANNOTATION_CLAUSE_SIZE]
                            == b"@def"
                    {
                        requires_end = true;
                        // PURELY METADATA IN REGARDS TO THE SRC
                        script_start = self.cursor;
                        // Limit is set to the maximum region size from the default READ_LIMIT size.
                        self.limit = chrn_utils::MAX_REGION_SIZE;
                        // Resets so that the max region size can be properly accounted for
                        self.bytes_consumed = 0;
                        self.ln_num_tracker.freeze();
                        self.col_tracker.freeze();
                        // Allows for `try_refill` to account for total turns in a way where it's
                        // restricted to 2 instead of 4. This is a less explicit way of just having
                        // an internal counter of `max_turns` where it would be more ! should that
                        // be done?
                        // self.state = SearchingState::InDef;
                        // Resetting so that the full 16KB can be consumed.
                        //
                        // self.reset();

                        // WARN:
                        // Stops at "f" because there may not be a byte after f, which should be
                        // handled by the main loop.
                        //
                        // Skips "@def" to the byte after it. This is safe since it will eithe
                        // return '\0' or `None` which both avoid over-indexing being a possibility
                        self.skip_unchecked(ANNOTATION_CLAUSE_SIZE);
                        // NOTE: SELF.POS IS 4 HERE

                        //WARN: This needs to be relative since only regions are used
                        // This is safe to hard-code because the condition itself only allows for
                        // this to be made if it's the first time @def is seen
                        let rel_start = 0;
                        let rel_end = 4;
                        def_span = Some(SourceSpan::new(
                            self.current_region_id,
                            rel_start,
                            // WARN: - 1 REMOVED FOR EXCLUSIVE SPANNING
                            rel_end,
                        ));

                        // Needs to continue or the last advance causes one-off errors since it
                        // conflicts with @def which already deals with itself
                        continue;
                    }

                    // This should not fail since if `@def` and `@end` fail, it should not consume
                    // anything in the process and just treat this as though it were a singular @ to
                    // skip
                    debug_assert_eq!(self.handle.buffer()[self.cursor], b'@');
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }
        dbg!(self.cursor);
        // TODO: Assert this...

        let region = self.create_region(script_start, None);
        // dbg!(str::from_utf8(&region.src_bytes));

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

            // (inclusive, exclusive) end
            //
            //  If we have "text\0", it advances "t" stopping at "\0", which naturally fits
            //  exclusive
            let eof_pos = self.cursor as u32;

            // Need to - 1 so that the spanning doesn't have a len of 0.
            // Cannot do + 1 to the end of the span or it extends one past len
            let eof_span = SourceSpan::new(self.current_region_id, eof_pos - 1, eof_pos);

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
    //FIX: These other paths do not account for if read limit has been passesd

    /// Returns a result instead of an option because if there are unclosed quotes and this method
    /// fails, it would need return a Some value which DOESN'T represent a failure, making it
    /// misleading.
    ///
    //// Returns `true` on success, `false` on failure (Maybe?)
    // TODO: LEXER SHOULD ALSO HANDLE THIS ALONE
    fn read_quotes(&mut self, quote_type: u8) -> Result<(), ()> {
        while let Some(b) = self.peek() {
            // IS this `OK`?

            match b {
                b'\\' => {
                    // If this isn't checked for then in the scenario "Hello\" it'll go past eof
                    // since it's assuming something is being escaped.

                    if self.peek_ahead(1).is_some() {
                        self.skip_unchecked(2);
                    } else {
                        self.advance();
                    }
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
    fn create_region(
        &mut self,
        script_start: usize,
        serial_start_opt: Option<usize>,
    ) -> SourceRegion {
        // self.persistent_buffer[script_start..self.cursor].to_vec(),
        SourceRegion::new(
            self.ln_num_tracker.val(),
            self.col_tracker.val(),
            self.handle.buffer()[script_start..self.cursor].to_vec(),
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
        let comment_start = self.cursor - 2;
        while let Some(current_byte) = self.peek()
            && depth > 0
        {
            if let Some(next_byte) = self.peek_ahead(1) {
                if current_byte == b'/' && next_byte == b'*' {
                    depth += 1;
                    self.skip_unchecked(2);
                } else if current_byte == b'*' && next_byte == b'/' {
                    depth -= 1;
                    self.skip_unchecked(2);
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

            let current_pos = self.cursor as u32;
            //WARN: + 1 EXCLUSIVE SPANNING CHANGE, current_pos -> current_pos + 1
            // Intended to allow it to at least cover one byte since its an exclusive span end
            let eof_span = SourceSpan::new(self.current_region_id, current_pos, current_pos + 1);

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

            // If a file really did hit eof during a multi-line comment the file is more likely than
            // not broken to even attempt to view. Also the lexer would get really really scared if
            // it had to deal with this.
            return Err(ConfigLoadError::Diagnostic(src_diag));
        }

        Ok(())
    }

    // This skip operation has only be used safely in this context. It's only used in scenarios like
    // multi-comments where look-ahead was already done to know that 2 bytes at most exist.
    fn skip_unchecked(&mut self, dest: usize) {
        self.cursor += dest;
        self.bytes_consumed += dest;
        self.col_tracker.increment_many(dest as u32);
    }

    fn advance(&mut self) -> Option<u8> {
        if self.bytes_consumed == self.limit {
            return None;
        }

        //TODO: Emit footer and return an err instead of `None` to notify caller
        // Since this just returns none it's more like an abrupt end of file at the user level.
        let b = self.peek();
        if b == Some(b'\n') {
            self.ln_num_tracker.increment();
            self.col_tracker.reset();
        } else {
            self.col_tracker.increment();
        }
        self.cursor += 1;
        self.bytes_consumed += 1;
        b
    }

    fn peek_ahead(&mut self, dest: usize) -> Option<u8> {
        if self.bytes_consumed == self.limit {
            return None;
        }

        self.handle.buffer().get(self.cursor + dest).copied()
    }

    fn peek(&mut self) -> Option<u8> {
        if self.bytes_consumed == self.limit {
            return None;
        }

        self.handle.buffer().get(self.cursor).copied()
    }

    //NOTE: WHAT IF: Say we have @ | def where the left and right are different buffer refills. If
    //we have a slice method, it needs to be able to take the @, and slice to
    //ANNOTATION_CLAUSE_SIZE, while not only keeping the previous byte span, but also ensuring the
    //byte span before that is kept because the @ could just be a random @.
    //So, what if there was either a working buffer, or local buffer for the slice method, where it
    //takes the @, keeps the previous previous buffer content in a sort of middle buffer, then
    //consume the rest of the slice's length, it returns a signal as to if it had enough to
    //slice/encountered an IO error, then we work with that buffer to have a more seamless slice
    //check past buffer len. Maybe we need to just keep 3 buffers by default since this check could
    //possibly be destructive since it could entail so many shifts to where it's a tiny API.

    //NOTE: This is going to be the general concept used eventually where, 2 16KB slices are rotated.
    //So, left gets 16 kb filled, moved to right, left gets filled, moved to right, if @def is
    //found, that context switches it to accept whatever the max region size is, say 32KB like right
    //now. The reason right now it's just a flat 64KB allocation is because streaming needed to be
    //learned before actually coming up with a tangibly good idea to, which seemed simple at first
    //but I was clearly wrong.

    //NOTE: STATE, self.pos = 5, both turns = 1
    // fn try_refill(&mut self) -> Result<(), ConfigLoaderOutput> {
    // if self.has_def {
    //     dbg!(self.current_turns, self.total_turns);
    //     dbg!(str::from_utf8(self.handle.buffer()));
    //     dbg!(self.handle.buffer()[self.pos + 1] as char);
    //     dbg!(self.pos, self.handle.buffer().len());
    //     panic!();
    // }
    // 1 turn == 8192 so 3 turns is 32 KB because self.pos needs to be at least 8192 to make
    // it here, which means it already contains what would be turn 4 inside of itself
    //
    // if self.total_turns == 4 {
    //     panic!("Total");
    //     debug_assert_eq!(self.has_def, false);
    //     return Ok(());
    // }

    // I believe this is @def with no @end
    // if (self.current_turns == 1 && self.has_def) {
    //     dbg!(self.cursor, self.writing_pos, self.absolute_pos);
    //     panic!();
    //     return Ok(());
    // }
    // If current turns == 1 and has def then it can't refill since it already has 16KB

    //NOTE: NOT TRUE because it could just be that the buffer needs to go back into the file
    //after a reset()
    //
    // Need to make sure it's eq to default size because if it stops at something like 2000,
    // that means it found eof, not that it should refill.
    // if self.cursor == self.handle.buffer().len()
    // && self.handle.buffer().len() == MAX_DEFAULT_BUFFER_SIZE
    // {
    //     return self.refill();
    //TEST:
    // match self.state {
    //     SearchingState::Searching => {
    //     }
    //     SearchingState::InDef => todo!(),
    // }
    // has_def acts as a boolean that determines of the max turns should be 2 or keep going
    // to 3
    //
    // This operation is moving the right slice to the left so that it acts as a
    // revolving state that removes any bytes processed if there is no @def
    //
    // if self.current_turns == 2 {
    //     debug_assert_eq!(self.persistent_buffer.len(), chrn_utils::MAX_REGION_SIZE);
    //     // Generically slices out the buffer we just used and puts it on the left
    //     let mid = self.persistent_buffer.len() / 2;
    //     let buf_len = self.handle.buffer().len();
    //     // dbg!(str::from_utf8(&self.total_buffer), self.total_buffer.len());
    //     // NOTE: Swapping left with right so that when the right is overwritten by the
    //     // refill it does not remove our current state of bytes
    //     let (left, right) = self.persistent_buffer.split_at_mut(mid);
    //     // dbg!(str::from_utf8(left), str::from_utf8(right));
    //     left.copy_from_slice(right);
    //     // dbg!(str::from_utf8(&self.total_buffer), self.total_buffer.len());
    //     // panic!();
    //     // dbg!(
    //     //     str::from_utf8(&self.total_buffer),
    //     //     str::from_utf8(self.handle.buffer())
    //     // );
    //     // self.current_turns = 0;
    // }

    // if self.has_def && self.current_turns == 0 {
    //     panic!();
    // }

    // if self.current_turns == 1 && self.has_def {
    //     panic!();
    //     // let (left, right) = self.total_buffer.split_at_mut(mid);
    //     // dbg!(str::from_utf8(left), str::from_utf8(right));
    //     // left.copy_from_slice(right);
    // }
    //     }
    //     Ok(())
    // }

    // fn refill(&mut self) -> Result<(), ConfigLoaderOutput> {
    //     let buf_len = self.handle.buffer().len();
    //     dbg!(buf_len);
    //
    //     let mut applying = self.writing_pos + buf_len;
    //     match self.state {
    //         SearchingState::Searching => {
    //             // Since region cannot exceed 16KB it just copies the buffer that was just read into it's
    //             // right slice
    //
    //             // If @def if present and this is true then a bug is present in regards to the caller of
    //             // refill. At no point should an @def have permission to exceed 16KB (I think)
    //             if applying >= chrn_utils::MAX_REGION_SIZE {
    //                 // If this weren't mutated it would still be positioned as though it was added to
    //                 // writing pos, which could lead to behavior like 0..MAX_REGION_SIZE given 2 8KB writes
    //                 applying -= self.writing_pos;
    //                 dbg!(applying);
    //                 self.writing_pos = 0;
    //             }
    //
    //             let write_slice = &mut self.persistent_buffer[self.writing_pos..applying];
    //             dbg!(str::from_utf8(write_slice));
    //             dbg!(
    //                 write_slice.len(),
    //                 self.handle.buffer().len(),
    //                 self.writing_pos
    //             );
    //             write_slice.copy_from_slice(self.handle.buffer());
    //             self.writing_pos = applying;
    //         }
    //         SearchingState::InDef => {
    //             dbg!(self.bytes_consumed);
    //             if self.bytes_consumed >= chrn_utils::MAX_REGION_SIZE {
    //                 todo!("New error");
    //                 return Ok(());
    //             }
    //
    //             let write_slice = &mut self.persistent_buffer[self.writing_pos..applying];
    //             write_slice.copy_from_slice(self.handle.buffer());
    //             self.writing_pos = applying;
    //         }
    //     }
    //     // dbg!(str::from_utf8(write_slice));
    //     // panic!();
    //     // debug_assert_eq!(
    //     //     &self.total_buffer[..MAX_DEFAULT_BUFFER_SIZE],
    //     //     self.handle.buffer(),
    //     //     "`total_buffer` is not aligned with handle.buffer() [total len = {}| handle len = {}]",
    //     //     self.total_buffer.len(),
    //     //     self.handle.buffer().len(),
    //     // );
    //
    //     self.handle.consume(buf_len);
    //
    //     debug_assert_eq!(
    //         self.persistent_buffer.len(),
    //         chrn_utils::MAX_REGION_SIZE,
    //         "total len = {}",
    //         self.persistent_buffer.len()
    //     );
    //
    //     match self.handle.fill_buf() {
    //         Ok(buf) => {
    //             dbg!(str::from_utf8(buf));
    //             if buf.len() > 0 {
    //                 // To ensure turns are only increased upon actual refills so that it does not over-expand
    //                 // So that when creating regions the pos also accounts for if the refill
    //                 // actually did anything
    //                 self.cursor = 0;
    //             }
    //             Ok(())
    //         }
    //         Err(err) => {
    //             let out = ConfigLoaderOutput::UnrecoverableErr(ConfigLoadError::IO(err));
    //             Err(out)
    //         }
    //     }
    // }

    //// All buffer iteration related variables are reset
    // fn reset(&mut self) {
    //     self.handle.consume(self.cursor);
    //     self.cursor = 0;
    //     // dbg!(str::from_utf8(&self.handle.buffer()[..]));
    //     self.bytes_consumed = 0;
    //     self.writing_pos = 0;
    //     // self.total_turns = 0;
    //     // self.current_turns = 0;
    //     self.persistent_buffer.fill(0);
    //     // match self.handle.fill_buf() {
    //     //     Ok(buf) => {
    //     //         dbg!(str::from_utf8(buf), buf.len());
    //     //     }
    //     //     Err(err) => {
    //     //         let out = ConfigLoaderOutput::UnrecoverableErr(ConfigLoadError::IO(err));
    //     //         return Err(out);
    //     //     }
    //     // };
    //     // Ok(())
    // }

    // Using turns instead of just position because this is easier to track?
    // fn advance_absolute(&mut self) {
    //     self.absolute_pos += 1;
    //     if self.absolute_pos % MAX_DEFAULT_BUFFER_SIZE == 0 {
    //         // self.advance_turns();
    //         self.absolute_pos = 0;
    //     }
    // }

    // fn skip_absolute(&mut self, amt: usize) {
    //     //NOTE: SELF POS IS 4 HERE
    //
    //     // Absolute pos can only be less than or eq to 8KB
    //     let applied = self.absolute_pos + amt;
    //     dbg!(applied);
    //
    //     // If subtracting would underflow set
    //     if applied >= MAX_DEFAULT_BUFFER_SIZE {
    //         let difference = applied - MAX_DEFAULT_BUFFER_SIZE;
    //         self.absolute_pos = difference;
    //         // self.advance_turns();
    //         dbg!(difference);
    //         panic!("wait");
    //     } else {
    //         // if self.has_def {
    //         //     dbg!(self.pos);
    //         //     dbg!(applied);
    //         //     panic!();
    //         // }
    //         self.absolute_pos = applied;
    //     }
    //
    //     // If difference is 0 then we reached buffer size otherwise
    // }
}

// Should probably put this in some sort of utils/utils file with obscure structures?
// Or maybe just a local config loader owned thing
/// Tracker that stores a "freeze" flag which takes the last bit in it's 32 bits, which allows it to stay 4
/// bytes instead of memory padding from a `bool`.
#[derive(Debug, Default)]
struct NumberTracker {
    inner: u32,
}

// Not necessary. But it would be 8 bytes which would cause the entire program to otherwise combust.
impl NumberTracker {
    const FREEZE_FLAG: u32 = 0x8000_0000;
    const VAL_MASK: u32 = 0x7FFF_FFFF;

    fn new(inner: u32) -> NumberTracker {
        NumberTracker { inner }
    }

    fn val(&self) -> u32 {
        self.inner & Self::VAL_MASK
    }

    fn increment(&mut self) {
        // If inner takes over val radius we instantly combust.
        if !self.is_frozen() {
            self.inner += 1;
        }
    }

    fn increment_many(&mut self, amt: u32) {
        if !self.is_frozen() {
            self.inner += amt;
        }
    }

    fn reset(&mut self) {
        if !self.is_frozen() {
            self.inner = 1;
        }
    }

    fn freeze(&mut self) {
        self.inner |= Self::FREEZE_FLAG;
    }

    fn is_frozen(&self) -> bool {
        (self.inner & Self::FREEZE_FLAG) != 0
    }
}

// Maybe when streaming is used this can be used
// enum InternalErr {
//     MaxTurns,
//     Diagnostic(SourceDiagnostic),
// }
//
// #[derive(Debug, PartialEq, Eq, Clone, Copy)]
// enum SearchingState {
//     Searching,
//     InDef,
// }
