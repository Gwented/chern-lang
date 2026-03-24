//FIXME: FIX REQUIRES END MESSAGE
use std::{
    io::{BufRead, BufReader, Read},
    ops::Range,
    path::{Path, PathBuf},
};

use crate::{core_error::ConfigLoadError, metadata::ChernMetadata, reporter, symbols::Span};

// TEST: Ignore this
const DEFINITION_SIZE: usize = 4;
const MAX_READ: usize = 1_000_000;

const READ_LIMIT_OFFSET: usize = 500;

// More inclusive name
//TEST: Suspicious lifetime
pub struct FileLoader<'a, R: Read> {
    // Configuration file path
    path: &'a Path,
    handle: BufReader<R>,
    pos: usize,
    lines_read: usize,
}

//NOTE: This forces paths to be given, but if the chern file itself doesn't have a path given
//then the language doesn't work anyways. May leave as is.
impl<R: Read> FileLoader<'_, R> {
    pub fn new(path: &Path, handle: R) -> FileLoader<'_, R> {
        FileLoader {
            path,
            handle: BufReader::new(handle),
            pos: 0,
            lines_read: 1,
        }
    }

    /// Returns a Success value of the bytes to Lex, the offset of where to start lexing if an
    /// `@def` is present, and the offset of where to start reading the serialized data if an
    /// `@def` and `@end` is present. Returns a `ConfigLoadError` upon failure that has internal
    /// error details.
    pub fn load_config(&mut self) -> Result<ChernMetadata, ConfigLoadError> {
        // Doesn't NEED definition but will error if declared and not closed
        let mut requires_end = false;

        let mut first_quote = None;
        let mut quotes_seen = 0;

        let mut lex_start = 0;

        let mut def_span = None;

        self.handle.fill_buf()?;

        while let Some(b) = self.peek() {
            if b == b'\0' {
                break;
            }

            let span_start = self.pos;

            match b {
                // Quote and comment handling is odd here because there is actual way to know where
                // the quote ended without. A probability model.
                b'"' | b'\'' => {
                    // Even though this can't fail
                    let quote_start = self.pos;
                    let quote_type = self.advance().unwrap_or(b'\0');

                    quotes_seen += 1;

                    let start_line = self.lines_read;

                    // Is there a reason for lines_read to be printed if there are multiple quotes?
                    // When are there ever NOT multiple quotes if it's in a serialized file?
                    if self.read_quotes(quote_type).is_err() {
                        let note = if quotes_seen > 1 {
                            "\nnote: There are other quotes within the file so the line given could be incorrect"
                        } else {
                            ""
                        };

                        let msg = format!(
                            "Found unclosed quotes at line {} which reached <eof>{}",
                            start_line, note
                        );

                        let spans = if quotes_seen > 1 {
                            let start = first_quote.expect("Already > 1");

                            let end = if self.pos + READ_LIMIT_OFFSET < self.handle.buffer().len() {
                                self.pos + READ_LIMIT_OFFSET
                            } else {
                                self.handle.buffer().len()
                            };

                            let search_range = start..end;

                            dbg!(self.handle.buffer()[first_quote.unwrap()] as char);

                            self.quote_start_probability(quote_type, search_range)
                        } else {
                            [Span::new(quote_start, quote_start)].to_vec()
                        };
                        dbg!(&spans);

                        dbg!(self.handle.buffer().len());

                        dbg!(str::from_utf8(
                            &self.handle.buffer()[spans[0].start..spans[0].end]
                        ));

                        let ln_data = reporter::form_err_diag(self.handle.buffer(), &spans, false);
                        let err_msg = reporter::standardize_err(&msg, &ln_data, "");

                        println!("{}", err_msg);
                        panic!("end");

                        return Err(ConfigLoadError::UnclosedQuotes(msg));
                    }

                    if quotes_seen == 1 {
                        first_quote = Some(quote_start)
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

                        return Ok(ChernMetadata::new(
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
                        def_span = Some(Span::new(span_start, self.pos));
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
            Ok(ChernMetadata::new(
                PathBuf::from(self.path),
                self.handle.buffer()[..self.pos].to_vec(),
                lex_start,
                // Some or None
                None,
            ))
        } else {
            let msg = format!(
                "Could not find `@end` after `@def` from path \"{}\" after reading {} line(s)",
                self.path.display(),
                self.lines_read
            );

            Err(ConfigLoadError::UnclosedDef(msg))
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

    fn handle_multi_comment(&mut self) -> Result<(), ConfigLoadError> {
        let mut depth = 1;

        let comment_start = self.lines_read;
        // - 1 to include the '//' or '/*' part of a comment
        let comment_span = Span::new(self.pos - 1, self.pos);

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
            let msg = format!(
                "Found unclosed multi-line comment in configuration file which started at line {}",
                comment_start
            );

            return Err(ConfigLoadError::UnclosedQuotes(msg));
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

    //TEST:
    //Needs to use same quote methods and @def mentions
    fn quote_start_probability(&self, q_type: u8, search_range: Range<usize>) -> Vec<Span> {
        let mut q_scores: Vec<QuoteScore> = Vec::new();
        // dbg!(&str::from_utf8(&self.handle.buffer()[search_range]));

        let mut score_pos: usize = 0;
        let mut state = State::SearchingForQuotes;
        let mut unclosed_len: usize = 0;

        let l_rate = 0.0001;

        //WARN: REMOVE CLONE WHEN DONE
        for (i, b) in self.handle.buffer()[search_range.clone()]
            .iter()
            .enumerate()
        {
            match state {
                State::SearchingForQuotes => {
                    dbg!(*b as char);
                    if *b == q_type {
                        score_pos = q_scores.len();

                        let q_score = QuoteScore::new(i, None, 0.05);
                        // dbg!(self.handle.buffer()[i] as char);
                        // dbg!(str::from_utf8(
                        //     &self.handle.buffer()[0 + search_range.start..0 + search_range.end]
                        // ));
                        // dbg!(&q_score);
                        q_scores.push(q_score);

                        state = State::InQuotes;
                        continue;
                    }
                }
                State::InQuotes => {
                    if *b != q_type {
                        let q_score = &mut q_scores[score_pos];

                        q_score.score += unclosed_len as f32 * (l_rate);

                        unclosed_len += 1;

                        // dbg!(&str::from_utf8(&self.handle.buffer()[i..search_range.end]));
                        // If looking for target and found target
                    } else if *b == q_type {
                        // Quotes are next
                        q_scores[score_pos].end_pos = Some(i);
                        unclosed_len = 0;

                        state = State::SearchingForQuotes;
                    }
                }
            }
        }

        // If more is done
        let probs: Vec<(usize, f32)> = Vec::new();

        dbg!(&q_scores);

        let mut res_idx: usize = 0;
        let mut largest_score = 0.0;

        for (i, score) in q_scores.iter().map(|q| q.score).enumerate() {
            if score > largest_score {
                largest_score = score;
                res_idx = i;
            }
        }

        // dbg!(&q_scores[res_idx]);

        let highest_q = &q_scores[res_idx];
        let start = highest_q.start_pos + search_range.start;

        let end = highest_q
            .end_pos
            .map(|v| v + search_range.start)
            .unwrap_or(highest_q.start_pos);

        // let highest_span = Span::new(start, end);

        dbg!(highest_q.start_pos, highest_q.end_pos);

        // Reporter does not correctly print the line when one span is given in this context. Cocerning but
        // not priority
        let first = Span::new(start, start);
        let last = Span::new(end, end);

        vec![first, last]
    }
}

enum State {
    SearchingForQuotes,
    InQuotes,
    // In def
}

#[derive(Debug)]
struct QuoteScore {
    start_pos: usize,
    end_pos: Option<usize>,
    score: f32,
}

impl QuoteScore {
    fn new(start_pos: usize, end_pos: Option<usize>, score: f32) -> QuoteScore {
        QuoteScore {
            start_pos,
            end_pos,
            score,
        }
    }
}
