//TODO: Maybe actually make the spans inclusive exclusive so that + 1 is not needed later
//and - 1 is not needed now
use chrn_utils::{
    intern::{self, Intern},
    keywords::{self, Keyword},
};
use common::span::Span;

use crate::{
    token::{Notation, SpannedToken, Token},
    trivia::{Trivia, TriviaKind},
};

const MAX_ILLEGAL_TOKS: u8 = 5;

// Bit-wise operations for read_num
const NOTATION_FLOAT: u8 = 1 << 0;
const NOTATION_HEX: u8 = 1 << 1;
const NOTATION_BIN: u8 = 1 << 2;
const NOTATION_OCTAL: u8 = 1 << 3;

pub struct Lexer<'a> {
    src_bytes: &'a [u8],
    pos: usize,
    trivia: Vec<Trivia>,
    trivia_start_idx: usize,
    trivia_end_idx: usize,
}

impl Lexer<'_> {
    /// Returns lexed tokens using a tuple with the tokens lexed, along with the trivia if needed
    /// for tooling
    // WARN: The file is fully dependent on being able to lex from a certain point so the @ confirmation
    // here should MAYBE be removed
    pub fn new(src: &[u8], script_start: usize) -> Lexer<'_> {
        Lexer {
            src_bytes: src,
            // Not even going to acknowledge what was here before
            pos: script_start,
            trivia: Vec::new(),
            trivia_start_idx: 0,
            trivia_end_idx: 0,
        }
    }

    pub fn tokenize(&mut self, interner: &mut Intern) -> (Vec<SpannedToken>, Vec<Trivia>) {
        let mut toks: Vec<SpannedToken> = Vec::new();

        // For threshold of illegal tokens before just giving up
        let mut illegal_toks: u8 = 0;

        // Could be removed
        // let mut in_def = false;

        loop {
            self.handle_trivia();

            if self.peek() == b'\0' || illegal_toks > MAX_ILLEGAL_TOKS {
                toks.push(SpannedToken {
                    tok: Token::EOF,
                    span: Span::new(self.pos, self.pos),
                    leading_trivia_indices: Span::new(self.trivia_start_idx, self.trivia_end_idx),
                });

                break;
            }

            let ch = self.peek_char();

            match ch {
                c if c.is_alphabetic() || c == '_' => {
                    toks.push(self.read_id(interner));
                }
                c if c.is_ascii_digit() => {
                    toks.push(self.read_num(interner));
                }
                ':' => {
                    let (start, mut end) = (self.pos, self.pos);

                    let tok = if self.peek_ahead(1) == b'=' {
                        self.advance();
                        end = self.pos;
                        self.advance();

                        Token::Walrus
                    } else {
                        self.advance();
                        Token::Colon
                    };

                    toks.push(SpannedToken {
                        tok,
                        span: Span::new(start, end),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
                    });
                }
                '(' => {
                    toks.push(SpannedToken {
                        tok: Token::OParen,
                        span: Span::new(self.pos, self.pos),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
                    });

                    self.advance();
                }
                ')' => {
                    toks.push(SpannedToken {
                        tok: Token::CParen,
                        span: Span::new(self.pos, self.pos),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
                    });

                    self.advance();
                }
                '<' => {
                    let (start, mut end) = (self.pos, self.pos);

                    let tok = if self.peek_ahead(1) == b'=' {
                        self.advance();
                        end = self.pos;
                        self.advance();

                        Token::LessOrEq
                    } else {
                        self.advance();
                        Token::OAngleBracket
                    };

                    toks.push(SpannedToken {
                        tok,
                        span: Span::new(start, end),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
                    });
                }
                '>' => {
                    let (start, mut end) = (self.pos, self.pos);

                    let tok = if self.peek_ahead(1) == b'=' {
                        self.advance();
                        end = self.pos;
                        self.advance();

                        Token::GreaterOrEq
                    } else {
                        self.advance();
                        Token::CAngleBracket
                    };

                    toks.push(SpannedToken {
                        tok,
                        span: Span::new(start, end),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
                    });
                }
                '[' => {
                    toks.push(SpannedToken {
                        tok: Token::OBracket,
                        span: Span::new(self.pos, self.pos),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
                    });

                    self.advance();
                }
                ']' => {
                    toks.push(SpannedToken {
                        tok: Token::CBracket,
                        span: Span::new(self.pos, self.pos),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
                    });

                    self.advance();
                }
                '{' => {
                    toks.push(SpannedToken {
                        tok: Token::OCurlyBracket,
                        span: Span::new(self.pos, self.pos),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
                    });

                    self.advance();
                }
                '}' => {
                    toks.push(SpannedToken {
                        tok: Token::CCurlyBracket,
                        span: Span::new(self.pos, self.pos),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
                    });

                    self.advance();
                }
                ',' => {
                    toks.push(SpannedToken {
                        tok: Token::Comma,
                        span: Span::new(self.pos, self.pos),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
                    });

                    self.advance();
                }
                '@' => {
                    // Allows for same behavior even in file with serialized data
                    if self.is_def_start() {
                        // in_def = true;
                        toks.push(SpannedToken {
                            tok: Token::Def,
                            span: Span::new(self.pos, self.pos + keywords::DEFINITION_SIZE - 1),
                            leading_trivia_indices: Span::new(
                                self.trivia_start_idx,
                                self.trivia_end_idx,
                            ),
                        });

                        self.skip(keywords::DEFINITION_SIZE);
                    } else if self.is_def_end() {
                        // in_def = false;

                        toks.push(SpannedToken {
                            tok: Token::End,
                            span: Span::new(self.pos, self.pos + keywords::DEFINITION_SIZE),
                            leading_trivia_indices: Span::new(
                                self.trivia_start_idx,
                                self.trivia_end_idx,
                            ),
                        });

                        // start_offset = self.pos + DEFINITION_SIZE;
                        break;
                    } else {
                        toks.push(SpannedToken {
                            tok: Token::At,
                            span: Span::new(self.pos, self.pos),
                            leading_trivia_indices: Span::new(
                                self.trivia_start_idx,
                                self.trivia_end_idx,
                            ),
                        });

                        self.advance();
                    }
                }
                '.' => {
                    let (start, mut end) = (self.pos, self.pos);

                    let tok = if self.peek_ahead(1) == b'.' && self.peek_ahead(2) == b'=' {
                        self.skip(2);
                        end = self.pos;
                        self.advance();

                        Token::DotRange
                    } else {
                        self.advance();
                        Token::Dot
                    };

                    toks.push(SpannedToken {
                        tok,
                        span: Span::new(start, end),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
                    })
                }
                '#' => {
                    toks.push(SpannedToken {
                        tok: Token::HashSymbol,
                        span: Span::new(self.pos, self.pos),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
                    });

                    self.advance();
                }
                '&' => {
                    let (start, mut end) = (self.pos, self.pos);

                    let tok = if self.peek_ahead(1) == b'&' {
                        self.advance();
                        end = self.pos;
                        self.advance();

                        Token::And
                    } else {
                        self.advance();
                        Token::Ampersand
                    };

                    toks.push(SpannedToken {
                        tok,
                        span: Span::new(start, end),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
                    });
                }
                '|' => {
                    let (start, mut end) = (self.pos, self.pos);

                    let tok = if self.peek_ahead(1) == b'|' {
                        self.advance();
                        // Getting the || within the span before skipping past the operator entirely
                        end = self.pos;
                        self.advance();

                        Token::Or
                    } else {
                        self.advance();
                        Token::VerticalBar
                    };

                    toks.push(SpannedToken {
                        tok,
                        span: Span::new(start, end),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
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
                    toks.push(SpannedToken {
                        tok: Token::Plus,
                        span: Span::new(self.pos, self.pos),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
                    });

                    self.advance();
                }
                '-' => {
                    let (start, mut end) = (self.pos, self.pos);

                    let token = if self.peek_ahead(1) == b'>' {
                        self.advance();
                        end = self.pos;
                        Token::SlimArrow
                    } else {
                        Token::Hyphen
                    };

                    toks.push(SpannedToken {
                        tok: token,
                        span: Span::new(start, end),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
                    });

                    self.advance();
                }
                '*' => {
                    toks.push(SpannedToken {
                        tok: Token::Asterisk,
                        span: Span::new(self.pos, self.pos),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
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
                    toks.push(SpannedToken {
                        tok: Token::Slash,
                        span: Span::new(self.pos, self.pos),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
                    });

                    self.advance();
                    // }
                }
                '=' => {
                    let (start, mut end) = (self.pos, self.pos);

                    let tok = if self.peek_ahead(1) == b'=' {
                        self.advance();
                        end = self.pos;
                        Token::EqualTo
                    } else {
                        Token::Assign
                    };

                    toks.push(SpannedToken {
                        tok,
                        span: Span::new(start, end),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
                    });

                    self.advance();
                }
                '~' => {
                    toks.push(SpannedToken {
                        tok: Token::Tilde,
                        span: Span::new(self.pos, self.pos),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
                    });

                    self.advance();
                }
                '!' => {
                    toks.push(SpannedToken {
                        tok: Token::ExclamationPoint,
                        span: Span::new(self.pos, self.pos),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
                    });

                    self.advance();
                }
                '?' => {
                    toks.push(SpannedToken {
                        tok: Token::QuestionMark,
                        span: Span::new(self.pos, self.pos),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
                    });

                    self.advance();
                }
                '%' => {
                    toks.push(SpannedToken {
                        tok: Token::Percent,
                        span: Span::new(self.pos, self.pos),
                        leading_trivia_indices: Span::new(
                            self.trivia_start_idx,
                            self.trivia_end_idx,
                        ),
                    });

                    self.advance();
                }
                _ => {
                    illegal_toks += 1;

                    toks.push(self.recover_illegal(None, interner));
                    if illegal_toks > MAX_ILLEGAL_TOKS {
                        // TODO: Maybe this should be at the end because technically @ is illegal too
                        eprintln!("Maximum illegal tokens found.\nReporting then aborting...");
                        // in_def = false;

                        toks.push(SpannedToken {
                            tok: Token::EOF,
                            span: Span::new(self.pos, self.pos),
                            leading_trivia_indices: Span::new(
                                self.trivia_start_idx,
                                self.trivia_end_idx,
                            ),
                        });

                        break;
                    }
                }
            }
        }

        let mut trivia: Vec<Trivia> = Vec::new();
        trivia.append(&mut self.trivia);
        (toks, trivia)
    }

    fn read_id(&mut self, interner: &mut Intern) -> SpannedToken {
        let mut start = self.pos;

        // e# for escape
        let is_escaped = if self.peek() == b'e' && self.peek_ahead(1) == b'#' {
            self.skip(2);
            start = self.pos;
            true
        } else {
            false
        };

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

        // Would it ever not be escaped if it's empty?
        // This means that we only found "e#" which is an error since it's an empty ident
        if id_str.is_empty() && is_escaped {
            return self.recover_illegal(Some(start - 2), interner);
        }

        let id = interner.intern(&id_str);

        // Offset due to advance being done before leaving the loop.
        let span = Span::new(start, end - 1);

        if id == intern::INTERNED_TRUE && !is_escaped {
            return SpannedToken {
                tok: Token::BoolLiteral(true),
                span,
                leading_trivia_indices: Span::new(self.trivia_start_idx, self.trivia_end_idx),
            };
        } else if id == intern::INTERNED_FALSE && !is_escaped {
            return SpannedToken {
                tok: Token::BoolLiteral(false),
                span,
                leading_trivia_indices: Span::new(self.trivia_start_idx, self.trivia_end_idx),
            };
        }

        match Keyword::try_from_interned_id(id) {
            Some(kw) if !is_escaped => SpannedToken {
                tok: Token::Keyword(kw),
                span,
                leading_trivia_indices: Span::new(self.trivia_start_idx, self.trivia_end_idx),
            },
            _ => SpannedToken {
                tok: Token::Id(id),
                span,
                leading_trivia_indices: Span::new(self.trivia_start_idx, self.trivia_end_idx),
            },
        }
    }

    //TODO: This defaults to i64 as of right now, but should stay interned in the future.
    // This could also be more readable by building up the string, but it's fine as is.
    // Unicode
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
                    tok: Token::Illegal(msg_id),
                    span: Span::new(start, end),
                    leading_trivia_indices: Span::new(self.trivia_start_idx, self.trivia_end_idx),
                };
            }
        };

        let (id_str, num_notation) =
            if (notation & (NOTATION_HEX | NOTATION_BIN | NOTATION_OCTAL)) != 0 {
                let digits_start = if raw_str.len() > 2 { 2 } else { 0 };
                let digits = &raw_str[digits_start..].replace('_', "");

                let (radix, num_notation) = if (notation & NOTATION_HEX) != 0 {
                    (16, Notation::Hex)
                } else if (notation & NOTATION_BIN) != 0 {
                    (2, Notation::Bin)
                } else {
                    (8, Notation::Octal)
                };

                let num = i64::from_str_radix(digits, radix).unwrap_or(0);
                (num.to_string(), num_notation)
            } else {
                (raw_str.replace('_', ""), Notation::Decimal)
            };

        let id = interner.intern(&id_str);

        if (notation & NOTATION_FLOAT) == 0 {
            SpannedToken {
                tok: Token::Integer(id, num_notation),
                // NOTE: Same read_id reasoning
                span: Span::new(start, end - 1),
                leading_trivia_indices: Span::new(self.trivia_start_idx, self.trivia_end_idx),
            }
        } else {
            SpannedToken {
                tok: Token::Float(id, num_notation),
                span: Span::new(start, end - 1),
                leading_trivia_indices: Span::new(self.trivia_start_idx, self.trivia_end_idx),
            }
        }
    }

    //TODO: Check if this still works if quotes are unclosed WITHOUT the loader
    // No
    // Please
    fn read_quotes(&mut self, interner: &mut Intern) -> SpannedToken {
        let start = self.pos;

        while self.pos < self.src_bytes.len() {
            match self.peek() {
                b'\\' => {
                    let escape_start = self.pos - 1;
                    self.advance();

                    if let Some(_) = self.read_escape() {
                    } else {
                        return self.recover_illegal(Some(escape_start), interner);
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

        let path_res = str::from_utf8(&self.src_bytes[start..end]);

        match path_res {
            Ok(p) => {
                let path_id = interner.intern(p);
                SpannedToken {
                    tok: Token::Str(path_id),
                    span: Span::new(start - 1, end),
                    leading_trivia_indices: Span::new(self.trivia_start_idx, self.trivia_end_idx),
                }
            }
            Err(_) => {
                let msg_id = interner.intern("<invalid UTF-8 in string literal>");

                SpannedToken {
                    tok: Token::Illegal(msg_id),
                    span: Span::new(start - 1, end),
                    leading_trivia_indices: Span::new(self.trivia_start_idx, self.trivia_end_idx),
                }
            }
        }
    }

    //TODO: Check if this still works if quotes are unclosed WITHOUT the loader
    fn read_char(&mut self, interner: &mut Intern) -> SpannedToken {
        //WARN: This offset is really DIRTY and should be fixed
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
                            return self.recover_illegal(Some(escape_start), interner);
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
            return self.recover_illegal(Some(start - 1), interner);
        }

        let end = self.pos - 1;

        match result_char {
            Some(ch) => SpannedToken {
                tok: Token::Char(ch),
                span: Span::new(start - 1, end),
                leading_trivia_indices: Span::new(self.trivia_start_idx, self.trivia_end_idx),
            },
            None => {
                let id = interner.intern("empty character literal");
                SpannedToken {
                    tok: Token::Illegal(id),
                    span: Span::new(start - 1, end),
                    leading_trivia_indices: Span::new(self.trivia_start_idx, self.trivia_end_idx),
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

    fn peek(&self) -> u8 {
        self.src_bytes.get(self.pos).copied().unwrap_or(b'\0')
    }

    fn is_def_start(&self) -> bool {
        if self.pos + 3 > self.src_bytes.len() {
            return false;
        }

        let possible_start = &self.src_bytes[self.pos..=self.pos + 3];

        if possible_start == "@def".as_bytes() {
            return true;
        }

        false
    }

    fn is_def_end(&mut self) -> bool {
        if self.pos + 3 > self.src_bytes.len() {
            return false;
        }

        let possible_end = &self.src_bytes[self.pos..=self.pos + 3];

        if possible_end == "@end".as_bytes() {
            return true;
        }

        false
    }

    fn recover_illegal(&mut self, start: Option<usize>, interner: &mut Intern) -> SpannedToken {
        let start = if let Some(s) = start { s } else { self.pos };

        while self.pos < self.src_bytes.len() && !self.peek_char().is_whitespace() {
            self.advance_char();
        }
        //WARN: Same behavior as read_id
        let end = self.pos;

        let err_str = str::from_utf8(&self.src_bytes[start..end])
            .expect("Cannot fail due to loop only accepting valid UTF-8");

        let id = interner.intern(&err_str);

        SpannedToken {
            tok: Token::Illegal(id),
            // Same offset reason as all other spans
            span: Span::new(start, end - 1),
            leading_trivia_indices: Span::new(self.trivia_start_idx, self.trivia_end_idx),
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

    // Spans are (inclusive, inclusive)
    fn handle_trivia(&mut self) {
        // This is (inclusive, exclusive) but may make this (inclusive, inclusive) for
        // consistency purposes
        self.trivia_start_idx = self.trivia.len();
        loop {
            let trivia_start = self.pos;
            match self.peek_char() {
                c if !c.is_control() && c.is_whitespace() => {
                    self.skip_whitespace();
                    let trivia_end = self.pos - 1;

                    self.trivia.push(Trivia::new(
                        TriviaKind::Whitespace,
                        Span::new(trivia_start, trivia_end),
                    ));
                }
                '/' => {
                    if self.peek_ahead(1) == b'/' {
                        self.skip(2);
                        self.handle_comment();
                        // THIS IS REAL
                        // Maintining (inclusive, inclusive) spans since it stops at \n
                        let trivia_end = self.pos - 1;

                        self.trivia.push(Trivia::new(
                            TriviaKind::SingleComment,
                            Span::new(trivia_start, trivia_end),
                        ));
                    } else if self.peek_ahead(1) == b'*' {
                        self.skip(2);
                        self.handle_multi_comment();
                        let trivia_end = self.pos - 1;

                        self.trivia.push(Trivia::new(
                            TriviaKind::MultiComment,
                            Span::new(trivia_start, trivia_end),
                        ));
                    } else {
                        break;
                    }
                }
                //TODO: Check if windows is ok
                '\r' if self.peek_ahead(1) == b'\n' => {
                    self.advance();

                    let trivia_end = self.pos;
                    self.trivia.push(Trivia::new(
                        TriviaKind::NewLine,
                        Span::new(trivia_start, trivia_end),
                    ));

                    self.advance();
                }
                '\n' => {
                    let trivia_end = self.pos;
                    self.trivia.push(Trivia::new(
                        TriviaKind::NewLine,
                        Span::new(trivia_start, trivia_end),
                    ));

                    self.advance();
                }
                '\t' => {
                    let trivia_end = self.pos;
                    self.trivia.push(Trivia::new(
                        TriviaKind::Tab,
                        Span::new(trivia_start, trivia_end),
                    ));

                    self.advance();
                }
                _ => break,
            }
        }

        self.trivia_end_idx = self.trivia.len();
    }

    fn skip_whitespace(&mut self) {
        while !self.peek().is_ascii_control() && self.peek().is_ascii_whitespace() {
            self.advance();
        }

        while !self.peek_char().is_control() && self.peek_char().is_whitespace() {
            self.advance_char();
        }
    }
}
