//NOTE: Loader enforces size constraints so the lexer doesn't have to track any activity to ensure
//the program is running safely
// At most there could be some indexing errors although unlikely

//WARN: LEXER HAS BEEN CHANGED TO USE (INCLUSIVE, EXCLUSIVE) SO THERE MAY BE BUGS LATER IF SOMETHING
//WAS MISSED. I DO NOT BELIEVE THERE ARE BUGS LEFT.

pub mod lexer_output;
pub mod token;
pub mod trivia;
// TODO: Maybe give this diagnostics
//  I don't know buddy

use chrn_utils::{
    chrn_config::{ChrnConfig, chrn_perf::ChrnPerfStage},
    id_types::SourceRegionId,
    intern::{self, Intern},
    source_map::source_span::SourceSpan,
};
use lang::keywords::{self, Keyword};

use crate::lexer::{
    lexer_output::LexerOutput,
    token::{Notation, SpannedToken, Token},
    trivia::{Trivia, TriviaKind},
};

const MAX_INVALID_TOKS: u8 = 12;

// Bit-wise operations for read_num
const NOTATION_FLOAT: u8 = 1 << 0;
const NOTATION_HEX: u8 = 1 << 1;
const NOTATION_BIN: u8 = 1 << 2;
const NOTATION_OCTAL: u8 = 1 << 3;

pub struct Lexer<'a> {
    // Should be &str
    src_bytes: &'a [u8],
    script_start: usize,
    pos: usize,
    current_region_id: SourceRegionId,
    cfg: &'a mut ChrnConfig,
    /// Invalid toks found count
    invalid_toks: u8,
    trivia: Vec<Trivia>,
    trivia_start_idx: usize,
    trivia_end_idx: usize,
}

impl Lexer<'_> {
    /// Returns a tuple with the lexed tokens and with the trivia which contains data regarding
    /// trailing whitespace, newlines, and others ok `Ok`.
    // WARN: The file is fully dependent on being able to lex from a certain point so the @ confirmation
    // here should MAYBE be removed
    pub fn new<'a>(
        current_region_id: SourceRegionId,
        src: &'a [u8],
        script_start: usize,
        cfg: &'a mut ChrnConfig,
    ) -> Lexer<'a> {
        // Trivia is very dense most of the time hence it weighs more
        let speculated_trivia = src.len() / 25;
        Lexer {
            current_region_id,
            src_bytes: src,
            script_start,
            // Not even going to acknowledge what was here before
            pos: 0,
            trivia: Vec::with_capacity(speculated_trivia),
            cfg,
            invalid_toks: 0,
            trivia_start_idx: 0,
            trivia_end_idx: 0,
        }
    }

    // Maybe this should just return Result since it'd take extra work to know if there are invalid
    // tokens and if it failed in any form
    /// Tokenizes `self.src_bytes`
    ///
    /// Returns a type of `LexerOutput` instead of a summary or result because the lexer does not
    /// emit errors or warns.
    ///
    /// This is so that the parser can emit more detailed errors regarding syntax AND the invalid
    /// tokens, as well as avoiding the parser reporting the same errors as the lexer since both act
    /// off the same token data.
    pub fn tokenize(&mut self, interner: &mut Intern) -> LexerOutput {
        self.cfg.perf_tracker_mut().start();
        // 40 bytes : 1 token
        //
        // The partitioning of 40 was chosen to account for the fact that tokens are compressed
        // bytes meaning there will of course be less, but also with the fact that the odds of there
        // being not even ONE token in 40 entire bytes is extremely unlikely.
        let speculated_toks = self.src_bytes.len() / 40;
        let mut toks: Vec<SpannedToken> = Vec::with_capacity(speculated_toks);

        // For threshold of invalid tokens before just giving up
        let mut invalid_toks: u8 = 0;

        // Could be removed
        // let mut in_def = false;

        loop {
            self.handle_trivia();

            if self.peek() == b'\0' || invalid_toks > MAX_INVALID_TOKS {
                // Over-indexes if not subtracted
                // Could be an empty file so needs saturation
                //WARN: EXCLUSIVE SPANNING. NO LONGER DOES - 1 FOR eof_pos
                // Ends at current spot since it'll always be last_byte + 1, which matches
                //BUG: If we are at the start of the file, and we see EOF, subtracting the start by
                //1 will be a breaking sub, and it won't actually have a feasible span.
                //Trying something.
                //Will probably just remove spanning from EOF.
                let eof_pos = self.src_bytes.len().saturating_sub(1) as u32;

                toks.push(SpannedToken {
                    tok: Token::EOF,
                    //WARN: CHANGED TO + 1
                    span: SourceSpan::new(self.current_region_id, eof_pos, eof_pos + 1),
                    leading_trivia_indices: self.trivia_start_idx as u32
                        ..self.trivia_end_idx as u32,
                });

                break;
            }

            let ch = self.peek_char();

            match ch {
                c if c.is_alphabetic() || c == '_' => {
                    toks.push(self.read_ident(interner));
                }
                c if c.is_ascii_digit() => {
                    toks.push(self.read_num(interner));
                }
                //TODO: Tests for these types of lexed tokens that are multi-layered
                ':' => {
                    // WARN: EXCLUSIVE SPANNING: self.pos is now + 1 so that it captures ":" since
                    // the end is exclusive
                    let start = self.pos as u32;
                    // New advance

                    //WARN: CHANGED TO SKIP
                    // :=
                    let tok = if self.peek_ahead(1) == b'=' {
                        self.skip(2);
                        Token::Walrus
                    } else if self.peek_ahead(1) == b':' {
                        // ::
                        self.skip(2);
                        Token::StaticAccess
                    } else {
                        // WARN: EXCLUSIVE SPANNING: ADVANCES HERE TO ENSURE END IS NOT EMPTY
                        self.advance();
                        Token::Colon
                    };

                    toks.push(SpannedToken {
                        tok,
                        span: SourceSpan::new(self.current_region_id, start, self.pos as u32),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });
                }
                '(' => {
                    let pos = self.pos as u32;
                    toks.push(SpannedToken {
                        tok: Token::OParen,
                        span: SourceSpan::new(self.current_region_id, pos, pos + 1),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });

                    self.advance();
                }
                ')' => {
                    let pos = self.pos as u32;
                    toks.push(SpannedToken {
                        tok: Token::CParen,
                        span: SourceSpan::new(self.current_region_id, pos, pos + 1),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });
                    self.advance();
                }
                '<' => {
                    let start = self.pos as u32;

                    //WARN: CHANGED TO SKIP
                    // <=
                    let tok = if self.peek_ahead(1) == b'=' {
                        self.skip(2);
                        Token::LessOrEq
                    } else {
                        self.advance();
                        Token::OAngleBracket
                    };

                    toks.push(SpannedToken {
                        tok,
                        span: SourceSpan::new(self.current_region_id, start, self.pos as u32),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });
                }
                '>' => {
                    let start = self.pos as u32;

                    // WARN: CHANGED TO SKIP
                    // >=
                    let tok = if self.peek_ahead(1) == b'=' {
                        self.skip(2);
                        Token::GreaterOrEq
                    } else {
                        self.advance();
                        Token::CAngleBracket
                    };

                    toks.push(SpannedToken {
                        tok,
                        span: SourceSpan::new(self.current_region_id, start, self.pos as u32),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });
                }
                '[' => {
                    let pos = self.pos as u32;
                    toks.push(SpannedToken {
                        tok: Token::OBracket,
                        span: SourceSpan::new(self.current_region_id, pos, pos + 1),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });

                    self.advance();
                }
                ']' => {
                    let pos = self.pos as u32;
                    toks.push(SpannedToken {
                        tok: Token::CBracket,
                        span: SourceSpan::new(self.current_region_id, pos, pos + 1),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });

                    self.advance();
                }
                '{' => {
                    let pos = self.pos as u32;
                    toks.push(SpannedToken {
                        tok: Token::OCurlyBracket,
                        span: SourceSpan::new(self.current_region_id, pos, pos + 1),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });

                    self.advance();
                }
                '}' => {
                    let pos = self.pos as u32;
                    toks.push(SpannedToken {
                        tok: Token::CCurlyBracket,
                        span: SourceSpan::new(self.current_region_id, pos, pos + 1),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });

                    self.advance();
                }
                ',' => {
                    let pos = self.pos as u32;
                    toks.push(SpannedToken {
                        tok: Token::Comma,
                        span: SourceSpan::new(self.current_region_id, pos, pos + 1),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });

                    self.advance();
                }
                '@' => {
                    // Allows for same behavior even in file with serialized data
                    if self.is_def_start() {
                        // in_def = true;
                        toks.push(SpannedToken {
                            tok: Token::Def,
                            span: SourceSpan::new(
                                self.current_region_id,
                                self.pos as u32,
                                // WARN: EXCLUSVIE SPANNING: NO LONGER USES - 1 ON
                                // ANNOTATION_CLAUSE_SIZE
                                (self.pos + keywords::REGION_CLAUSE_SIZE) as u32,
                            ),
                            leading_trivia_indices: self.trivia_start_idx as u32
                                ..self.trivia_end_idx as u32,
                        });

                        self.skip(keywords::REGION_CLAUSE_SIZE);
                    } else if self.is_def_end() {
                        // in_def = false;

                        toks.push(SpannedToken {
                            tok: Token::End,
                            span: SourceSpan::new(
                                self.current_region_id,
                                self.pos as u32,
                                // WARN: EXCLUSVIE SPANNING: NO LONGER USES - 1 ON
                                // ANNOTATION_CLAUSE_SIZE
                                (self.pos + keywords::REGION_CLAUSE_SIZE) as u32,
                            ),
                            leading_trivia_indices: self.trivia_start_idx as u32
                                ..self.trivia_end_idx as u32,
                        });

                        // start_offset = self.pos + DEFINITION_SIZE;
                        break;
                    } else {
                        let pos = self.pos as u32;
                        toks.push(SpannedToken {
                            tok: Token::At,
                            span: SourceSpan::new(self.current_region_id, pos, pos + 1),
                            leading_trivia_indices: self.trivia_start_idx as u32
                                ..self.trivia_end_idx as u32,
                        });

                        self.advance();
                    }
                }
                '.' => {
                    let start = self.pos as u32;

                    // ..=
                    let tok = if self.peek_ahead(1) == b'.' && self.peek_ahead(2) == b'=' {
                        //WARN: EXCLUSIVE SPANNING:
                        self.skip(3);
                        Token::DotRangeInclusive
                    } else {
                        self.advance();
                        Token::Dot
                    };

                    toks.push(SpannedToken {
                        tok,
                        span: SourceSpan::new(self.current_region_id, start, self.pos as u32),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });
                }
                '#' => {
                    let pos = self.pos as u32;
                    toks.push(SpannedToken {
                        tok: Token::HashSymbol,
                        span: SourceSpan::new(self.current_region_id, pos, pos + 1),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });

                    self.advance();
                }
                '&' => {
                    let start = self.pos as u32;

                    // &&
                    let tok = if self.peek_ahead(1) == b'&' {
                        self.skip(2);
                        Token::And
                    } else {
                        self.advance();
                        Token::Ampersand
                    };

                    toks.push(SpannedToken {
                        tok,
                        span: SourceSpan::new(self.current_region_id, start, self.pos as u32),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });
                }
                '|' => {
                    let start = self.pos as u32;

                    // ||
                    let tok = if self.peek_ahead(1) == b'|' {
                        //WARN: EXCLUSIVE SPANNING:
                        self.skip(2);
                        Token::Or
                    } else {
                        self.advance();
                        Token::VerticalBar
                    };

                    toks.push(SpannedToken {
                        tok,
                        span: SourceSpan::new(self.current_region_id, start, self.pos as u32),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });
                }
                '"' => {
                    self.advance();
                    toks.push(self.read_quotes(interner));
                }
                '\'' => {
                    self.advance();
                    toks.push(self.read_char(interner));
                }
                '+' => {
                    let pos = self.pos as u32;
                    toks.push(SpannedToken {
                        tok: Token::Plus,
                        span: SourceSpan::new(self.current_region_id, pos, pos + 1),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });

                    self.advance();
                }
                '-' => {
                    let start = self.pos as u32;

                    // ->
                    let token = if self.peek_ahead(1) == b'>' {
                        self.skip(2);
                        Token::SlimArrow
                    } else {
                        self.advance();
                        Token::Hyphen
                    };

                    toks.push(SpannedToken {
                        tok: token,
                        span: SourceSpan::new(self.current_region_id, start, self.pos as u32),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });
                }
                '*' => {
                    let pos = self.pos as u32;
                    toks.push(SpannedToken {
                        tok: Token::Asterisk,
                        span: SourceSpan::new(self.current_region_id, pos, pos + 1),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });

                    self.advance();
                }
                '^' => {
                    let pos = self.pos as u32;
                    toks.push(SpannedToken {
                        tok: Token::Caret,
                        span: SourceSpan::new(self.current_region_id, pos, pos + 1),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });

                    self.advance();
                }
                // Trivia handles comment possbibilites
                '/' => {
                    // if self.peek_ahead(1) == b'/' {
                    //     self.skip(2);
                    //     self.handle_comment();
                    // } else if self.peek_ahead(1) == b'*' {
                    //     self.skip(2);
                    //     self.handle_multi_comment();
                    // } else {
                    let pos = self.pos as u32;
                    toks.push(SpannedToken {
                        tok: Token::Slash,
                        span: SourceSpan::new(self.current_region_id, pos, pos + 1),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });

                    self.advance();
                    // }
                }
                '=' => {
                    let start = self.pos as u32;

                    // ==
                    let tok = if self.peek_ahead(1) == b'=' {
                        self.skip(2);
                        Token::EqualTo
                    } else if self.peek_ahead(1) == b'>' {
                        // =>
                        self.skip(2);
                        Token::NotSlimArrow
                    } else {
                        self.advance();
                        Token::Assign
                    };

                    toks.push(SpannedToken {
                        tok,
                        span: SourceSpan::new(self.current_region_id, start, self.pos as u32),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });
                }
                '~' => {
                    let pos = self.pos as u32;
                    toks.push(SpannedToken {
                        tok: Token::Tilde,
                        span: SourceSpan::new(self.current_region_id, pos, pos + 1),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });

                    self.advance();
                }
                '!' => {
                    let start = self.pos as u32;
                    let tok = if self.peek_ahead(1) == b'=' {
                        self.skip(2);
                        Token::NotEq
                    } else {
                        self.advance();
                        Token::ExclamationPoint
                    };

                    toks.push(SpannedToken {
                        tok,
                        span: SourceSpan::new(self.current_region_id, start, self.pos as u32),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });
                }
                '?' => {
                    let pos = self.pos as u32;
                    toks.push(SpannedToken {
                        tok: Token::QuestionMark,
                        span: SourceSpan::new(self.current_region_id, pos, pos + 1),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });

                    self.advance();
                }
                '%' => {
                    let pos = self.pos as u32;
                    toks.push(SpannedToken {
                        tok: Token::Percent,
                        span: SourceSpan::new(self.current_region_id, pos, pos + 1),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    });

                    self.advance();
                }
                _ => {
                    invalid_toks += 1;

                    toks.push(self.recover_invalid(None, interner));
                    if invalid_toks > MAX_INVALID_TOKS {
                        // TODO: Maybe this should be at the end because technically @ is invalid too
                        // Is this still needed?
                        eprintln!("Maximum invalid tokens found.\nReporting then aborting...");
                        // in_def = false;

                        toks.push(SpannedToken {
                            tok: Token::EOF,
                            span: SourceSpan::new(
                                self.current_region_id,
                                self.pos as u32,
                                self.pos as u32 + 1,
                            ),
                            leading_trivia_indices: self.trivia_start_idx as u32
                                ..self.trivia_end_idx as u32,
                        });

                        break;
                    }
                }
            }
        }

        self.cfg.perf_tracker_mut().stop(ChrnPerfStage::Lexer);

        let mut trivia: Vec<Trivia> = Vec::with_capacity(self.trivia.len());
        trivia.append(&mut self.trivia);
        LexerOutput::new(toks, trivia, self.invalid_toks)
    }

    /// Reads a string of characters based off of language expected heuristics
    /// May return an invalid token
    fn read_ident(&mut self, interner: &mut Intern) -> SpannedToken {
        let mut start = self.pos;

        // e# for escape
        let is_escaped = if self.peek() == b'e' && self.peek_ahead(1) == b'#' {
            self.skip(2);
            // if "e#something" starts at "something"
            start = self.pos;
            true
        } else {
            false
        };

        while (self.pos < self.src_bytes.len() && self.peek_char().is_alphanumeric())
            || (self.pos < self.src_bytes.len() && self.peek() == b'_')
        {
            self.advance_char();
        }

        // Advances at end so its end + 1 naturally, fitting exclusive span ends
        let end = self.pos;
        // Enforces utf-8 but module paths themselves don't need to be valid utf-8, am I

        // hallucinating?
        let id_str = match str::from_utf8(&self.src_bytes[start..end]) {
            Ok(s) => s,
            Err(_) => {
                return self.recover_invalid(start.into(), interner);
            }
        };

        // Would it ever not be escaped if it's empty?
        // This means that we only found "e#" which is an error since it's an empty ident
        if id_str.is_empty() && is_escaped {
            return self.recover_invalid(Some(start - 2), interner);
        }

        let interned_id = interner.intern(&id_str);

        // The start is different on escape so that it captures the "e#" can be shown in spanning
        let span_start = if is_escaped { start - 2 } else { start } as u32;

        // Spanning is purely visual and for external tooling, hence the "e#" in escape is still
        // kept inside the span, just as quotes are kept inside their span in `read_quotes`
        //WARN: USES END WITHOUT - 1
        let span = SourceSpan::new(self.current_region_id, span_start, end as u32);

        if interned_id.id == intern::INTERNED_TRUE && !is_escaped {
            return SpannedToken {
                tok: Token::BoolLiteral(true),
                span,
                leading_trivia_indices: self.trivia_start_idx as u32..self.trivia_end_idx as u32,
            };
        } else if interned_id.id == intern::INTERNED_FALSE && !is_escaped {
            return SpannedToken {
                tok: Token::BoolLiteral(false),
                span,
                leading_trivia_indices: self.trivia_start_idx as u32..self.trivia_end_idx as u32,
            };
        }

        match Keyword::try_from_interned_id(interned_id) {
            Some(kw) if !is_escaped => SpannedToken {
                tok: Token::Keyword(kw),
                span,
                leading_trivia_indices: self.trivia_start_idx as u32..self.trivia_end_idx as u32,
            },
            _ => SpannedToken {
                tok: Token::Id(interned_id),
                span,
                leading_trivia_indices: self.trivia_start_idx as u32..self.trivia_end_idx as u32,
            },
        }
    }

    //TODO: This defaults to i64 as of right now, but should stay interned in the future.
    // This could also be more readable by building up the string, but it's fine as is.
    // Unicode
    /// Reads a string of characters and attempts to interpret it as a numeric value
    /// based off of language expected heuristics.
    ///
    /// May return an invalid token.
    fn read_num(&mut self, interner: &mut Intern) -> SpannedToken {
        let start = self.pos;

        let mut notation: u8 = 0;

        if self.peek() == b'0' && self.peek_ahead(1) == b'x' {
            notation |= NOTATION_HEX;
            self.skip(2);
        } else if self.peek() == b'0' && self.peek_ahead(1) == b'b' {
            notation |= NOTATION_BIN;
            self.skip(2);
        } else if self.peek() == b'0' && self.peek_ahead(1) == b'o' {
            notation |= NOTATION_OCTAL;
            self.skip(2);
        }

        while self.pos < self.src_bytes.len() {
            match self.peek() {
                b'a'..=b'f' | b'A'..=b'F' if (notation & NOTATION_HEX) != 0 => {
                    self.advance();
                }
                b'0' | b'1' if (notation & NOTATION_BIN) != 0 => {
                    self.advance();
                }
                b'0'..=b'7' if (notation & NOTATION_OCTAL) != 0 => {
                    self.advance();
                }
                b'0'..=b'9' => {
                    self.advance();
                }
                // May remove '+' being usable
                b'e' if (notation & (NOTATION_HEX | NOTATION_BIN | NOTATION_OCTAL)) == 0 => {
                    let next = self.peek_ahead(1);

                    if (next == b'+' || next == b'-') && self.peek_ahead(2).is_ascii_digit() {
                        notation |= NOTATION_FLOAT;
                        self.skip(2);
                    } else if next.is_ascii_digit() {
                        notation |= NOTATION_FLOAT;
                        self.advance();
                    } else {
                        break;
                    }
                }
                b'.' if (notation & NOTATION_FLOAT) == 0
                    && (notation & (NOTATION_HEX | NOTATION_BIN | NOTATION_OCTAL)) == 0
                    && self.peek_ahead(1) != b'.'
                    // Maybe this will be possible, but it looks weird.
                    && self.peek_ahead(1).is_ascii_digit() =>
                {
                    notation |= NOTATION_FLOAT;
                    self.advance();
                }
                //NOTE: Checking if next could be "..=" to avoid collision. Could be better. Maybe.
                b'.' if (notation & NOTATION_FLOAT) == 0 && self.peek_ahead(1) == b'.' => break,
                b'_' => {
                    self.advance();
                }
                _ => break,
            }
        }

        let end = self.pos;

        let raw_str = match str::from_utf8(&self.src_bytes[start..end]) {
            Ok(val) => val,
            Err(_) => {
                // NOTE: I don't actually think this is possible. Like at all.
                let msg_id = interner.intern("<invalid ASCII in numeric>");
                return SpannedToken {
                    tok: Token::Invalid(msg_id),
                    span: SourceSpan::new(self.current_region_id, start as u32, end as u32),
                    leading_trivia_indices: self.trivia_start_idx as u32
                        ..self.trivia_end_idx as u32,
                };
            }
        };

        let (id_str, num_notation) =
            if (notation & (NOTATION_HEX | NOTATION_BIN | NOTATION_OCTAL)) != 0 {
                let digits = raw_str[2..].replace('_', "");

                if digits.is_empty() {
                    let msg_id = interner.intern("<empty numeric literal>");
                    return SpannedToken {
                        tok: Token::Invalid(msg_id),
                        span: SourceSpan::new(self.current_region_id, start as u32, end as u32),
                        leading_trivia_indices: self.trivia_start_idx as u32
                            ..self.trivia_end_idx as u32,
                    };
                }

                let (radix, num_notation) = if (notation & NOTATION_HEX) != 0 {
                    (16, Notation::Hex)
                } else if (notation & NOTATION_BIN) != 0 {
                    (2, Notation::Bin)
                } else {
                    (8, Notation::Octal)
                };

                let num = match i64::from_str_radix(&digits, radix) {
                    Ok(n) => n,
                    Err(_) => {
                        let msg_id = interner.intern("<invalid numeric literal>");
                        return SpannedToken {
                            tok: Token::Invalid(msg_id),
                            span: SourceSpan::new(self.current_region_id, start as u32, end as u32),
                            leading_trivia_indices: self.trivia_start_idx as u32
                                ..self.trivia_end_idx as u32,
                        };
                    }
                };
                (num.to_string(), num_notation)
            } else {
                (raw_str.replace('_', ""), Notation::Decimal)
            };

        let id = interner.intern(&id_str);

        //WARN: NO END - 1 TO FIX EXCLUSIVE END
        let span = SourceSpan::new(self.current_region_id, start as u32, end as u32);

        if (notation & NOTATION_FLOAT) == 0 {
            SpannedToken {
                tok: Token::Integer(id, num_notation),
                // NOTE: Same read_id reasoning
                span,
                leading_trivia_indices: self.trivia_start_idx as u32..self.trivia_end_idx as u32,
            }
        } else {
            SpannedToken {
                tok: Token::Float(id, num_notation),
                span,
                leading_trivia_indices: self.trivia_start_idx as u32..self.trivia_end_idx as u32,
            }
        }
    }

    //TODO: Check if this still works if quotes are unclosed WITHOUT the loader
    // No
    // Please
    /// Reads bytes until an end quote is reached. May return an invalid token.
    fn read_quotes(&mut self, interner: &mut Intern) -> SpannedToken {
        let start = self.pos;

        while self.pos < self.src_bytes.len() {
            match self.peek() {
                b'\\' => {
                    let escape_start = self.pos - 1;
                    self.advance();

                    if let Some(_) = self.read_escape() {
                    } else {
                        return self.recover_invalid(Some(escape_start), interner);
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

        //WARN: IS - 1 BECAUSE THE QUOTE IS SKIPPED PAST
        //"Hello"\"e" -> removing the "e" "Hello\"" now it probably sets up for an exclusive end
        let end = self.pos - 1;

        let quotes_res = str::from_utf8(&self.src_bytes[start..end]);

        // This is to keep the quotes captured
        //WARN: CHANGED END TO END + 1 SO THAT IT MATCHES END QUOTE CAPTURING BEHAVIOR FOR SPANNING
        let span = SourceSpan::new(self.current_region_id, (start - 1) as u32, (end + 1) as u32);

        match quotes_res {
            Ok(p) => {
                let interned_id = interner.intern(p);
                SpannedToken {
                    tok: Token::Str(interned_id),
                    span,
                    leading_trivia_indices: self.trivia_start_idx as u32
                        ..self.trivia_end_idx as u32,
                }
            }
            Err(_) => {
                let msg_id = interner.intern("<invalid UTF-8 in string literal>");

                SpannedToken {
                    tok: Token::Invalid(msg_id),
                    span,
                    leading_trivia_indices: self.trivia_start_idx as u32
                        ..self.trivia_end_idx as u32,
                }
            }
        }
    }

    //TODO: Check if this still works if quotes are unclosed WITHOUT the loader
    fn read_char(&mut self, interner: &mut Intern) -> SpannedToken {
        // Starts 1 after single quote
        let start = self.pos;

        let mut result_char: Option<char> = None;
        let mut char_count: usize = 0;

        while self.pos < self.src_bytes.len() {
            match self.peek() {
                b'\\' => {
                    let escape_start = self.pos - 1;
                    self.advance();

                    match self.read_escape() {
                        Some(ch) => {
                            result_char = Some(ch);
                            char_count += 1;
                        }
                        None => {
                            return self.recover_invalid(Some(escape_start), interner);
                        }
                    }
                }
                b'\'' => {
                    self.advance();
                    break;
                }
                _ => {
                    let ch = self.peek_char();
                    result_char = Some(ch);

                    char_count += 1;

                    self.advance_char();
                }
            }
        }

        if char_count > 1 {
            // To start at the actual single quote start
            return self.recover_invalid(Some(start - 1), interner);
        }

        //WARN: REMOVED END - 1 SINCE ITS EXCLUSIVE AND + 1 IS INTENDED
        let end = self.pos;

        // start - 1 to capture first single quote
        let span = SourceSpan::new(self.current_region_id, (start - 1) as u32, end as u32);

        match result_char {
            Some(ch) => SpannedToken {
                tok: Token::Char(ch),
                span,
                leading_trivia_indices: self.trivia_start_idx as u32..self.trivia_end_idx as u32,
            },
            None => {
                let id = interner.intern("empty character literal");
                SpannedToken {
                    tok: Token::Invalid(id),
                    span,
                    leading_trivia_indices: self.trivia_start_idx as u32
                        ..self.trivia_end_idx as u32,
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
                    let digit = Self::hex_val(self.peek())?;
                    val = (val << 4) | digit;
                    self.advance();
                    count += 1;
                }

                if Self::hex_val(self.peek()).is_some() {
                    None
                } else {
                    Some(val as char)
                }
            }
            b'u' => {
                if self.peek_ahead(1) != b'{' {
                    return None;
                }
                self.skip(2);

                let mut val: u32 = 0;
                let mut count: usize = 0;

                loop {
                    let c = self.peek();

                    if c == b'}' && count > 0 {
                        self.advance();
                        break;
                    }

                    let digit = Self::hex_val(c)? as u32;
                    val = (val << 4) | digit;
                    self.advance();
                    count += 1;

                    if count > 6 {
                        return None;
                    }
                }

                char::from_u32(val)
            }
            _ => None,
        }
    }

    fn hex_val(c: u8) -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            b'A'..=b'F' => Some(c - b'A' + 10),
            _ => None,
        }
    }

    fn peek(&self) -> u8 {
        self.src_bytes.get(self.pos).copied().unwrap_or(b'\0')
    }

    fn is_def_start(&self) -> bool {
        if self.pos + 3 >= self.src_bytes.len() {
            return false;
        }

        let possible_start = &self.src_bytes[self.pos..self.pos + keywords::REGION_CLAUSE_SIZE];

        if possible_start == "@def".as_bytes() {
            return true;
        }

        false
    }

    fn is_def_end(&mut self) -> bool {
        if self.pos + 3 >= self.src_bytes.len() {
            return false;
        }

        let possible_end = &self.src_bytes[self.pos..=self.pos + 3];

        if possible_end == "@end".as_bytes() {
            return true;
        }

        false
    }

    fn recover_invalid(&mut self, start: Option<usize>, interner: &mut Intern) -> SpannedToken {
        self.invalid_toks += 1;
        let start = if let Some(s) = start { s } else { self.pos };

        while self.pos < self.src_bytes.len() && !self.peek_char().is_whitespace() {
            self.advance_char();
        }
        //WARN: Same behavior as read_id
        let end = self.pos;
        let err_str = String::from_utf8_lossy(&self.src_bytes[start..end]);

        let id = interner.intern(&err_str);

        SpannedToken {
            tok: Token::Invalid(id),
            // Same offset reason as all other spans
            // WARN: Could this be an error at some poitn where end - 1 < 0?
            //WARN: CHANGED FOR EXCLUSIVE
            span: SourceSpan::new(self.current_region_id, start as u32, end as u32),
            leading_trivia_indices: self.trivia_start_idx as u32..self.trivia_end_idx as u32,
        }
    }

    fn peek_char(&mut self) -> char {
        let b = self.peek();

        if b <= 127 {
            return b as char;
        }

        let chunk = &self.src_bytes[self.pos..];

        // Should be a test for this this is suspicious
        // Lazy evaluation to avoid utf-8 checking entire self.bytes
        //TODO: Handle this please!
        //No, we silently return null bytes.
        //Oh ok
        std::str::from_utf8(chunk)
            .ok()
            .and_then(|c| c.chars().next())
            .unwrap_or('\0')
    }

    fn handle_comment(&mut self) {
        while self.pos < self.src_bytes.len() && self.peek() != b'\n' {
            self.advance();
        }
    }

    //NOTE: Keeps depth tracked even though the loader would take care of this. Could change.
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
    }

    // May return byte
    // WARN: This could be an issue. Many other places alike are present.
    fn skip(&mut self, dest: usize) {
        self.pos += dest;
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

    fn advance_char(&mut self) -> char {
        let ch = self.peek_char();

        self.pos += ch.len_utf8();

        ch
    }

    /// Produces (inclusive, exclusive) ranged trivia tokens so that accurate information such as
    /// whitespace positioning, new lines, etc, are all tracked from the source.
    fn handle_trivia(&mut self) {
        // This is (inclusive, exclusive)
        self.trivia_start_idx = self.trivia.len();
        loop {
            let trivia_start = self.pos;
            match self.peek_char() {
                c if !c.is_control() && c.is_whitespace() => {
                    self.skip_whitespace();

                    self.trivia.push(Trivia::new(
                        TriviaKind::Whitespace,
                        SourceSpan::new(
                            self.current_region_id,
                            trivia_start as u32,
                            self.pos as u32,
                        ),
                    ));
                }
                '/' => {
                    if self.peek_ahead(1) == b'/' {
                        self.skip(2);
                        self.handle_comment();
                        // THIS IS REAL
                        // Maintaining (inclusive, exclusive)
                        // let trivia_end = self.pos;

                        self.trivia.push(Trivia::new(
                            TriviaKind::SingleComment,
                            SourceSpan::new(
                                self.current_region_id,
                                trivia_start as u32,
                                self.pos as u32,
                            ),
                        ));
                    } else if self.peek_ahead(1) == b'*' {
                        self.skip(2);
                        self.handle_multi_comment();

                        self.trivia.push(Trivia::new(
                            TriviaKind::MultiComment,
                            SourceSpan::new(
                                self.current_region_id,
                                trivia_start as u32,
                                self.pos as u32,
                            ),
                        ));
                    } else {
                        break;
                    }
                }
                //TODO: Check if windows is ok
                '\r' if self.peek_ahead(1) == b'\n' => {
                    // self.advance();
                    // Changed to singular skip
                    self.skip(2);

                    self.trivia.push(Trivia::new(
                        TriviaKind::Newline,
                        SourceSpan::new(
                            self.current_region_id,
                            trivia_start as u32,
                            self.pos as u32,
                        ),
                    ));
                }
                '\n' => {
                    self.advance();
                    self.trivia.push(Trivia::new(
                        TriviaKind::Newline,
                        SourceSpan::new(
                            self.current_region_id,
                            trivia_start as u32,
                            self.pos as u32,
                        ),
                    ));
                }
                '\t' => {
                    self.advance();
                    self.trivia.push(Trivia::new(
                        TriviaKind::Tab,
                        SourceSpan::new(
                            self.current_region_id,
                            trivia_start as u32,
                            self.pos as u32,
                        ),
                    ));
                }
                _ => break,
            }
        }

        self.trivia_end_idx = self.trivia.len();
    }

    /// Using UTF-8 standards, skips whitespace until either a control character is reached or
    /// a non-whitespace character is reached.
    fn skip_whitespace(&mut self) {
        while !self.peek().is_ascii_control() && self.peek().is_ascii_whitespace() {
            self.advance();
        }

        while !self.peek_char().is_control() && self.peek_char().is_whitespace() {
            self.advance_char();
        }
    }
}
