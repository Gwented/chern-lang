use common::{intern::Intern, symbols::Span};

use crate::token::{SpannedToken, Token};

/// Known size in bytes for `@def` and `@end`
const DEFINITION_SIZE: usize = 4;
const MAX_ILLEGAL_TOKS: u8 = 7;

pub struct Lexer<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Lexer<'_> {
    // WARN: The file is fully dependent on being able to lex from a certain point so the @ confirmation
    // here should MAYBE be removed
    pub fn new(bytes: &[u8], lex_start: usize) -> Lexer<'_> {
        Lexer {
            bytes,
            pos: 0 + lex_start,
        }
    }

    pub fn tokenize(&mut self, interner: &mut Intern) -> Vec<SpannedToken> {
        let mut tokens: Vec<SpannedToken> = Vec::new();

        // For threshold of illegal tokens before just giving up. Likely 8 cap.
        let mut illegal_toks: u8 = 0;

        // Could be removed
        let mut in_def = false;

        loop {
            self.skip_whitespace();

            if self.peek() == b'\0' || illegal_toks > MAX_ILLEGAL_TOKS {
                tokens.push(SpannedToken {
                    token: Token::EOF,
                    span: Span::new(self.pos, self.pos),
                });

                break;
            }

            //WARN: Ascii only
            let byte = self.peek();

            dbg!(byte as char);
            match byte {
                b if b.is_ascii_alphabetic() || b == b'_' => {
                    tokens.push(self.read_id(interner));
                }
                b if b.is_ascii_digit() => {
                    tokens.push(self.read_num(interner));
                }
                b':' => {
                    let (start, mut end) = (self.pos, self.pos);

                    let tok = if self.peek_ahead(1) == b'=' {
                        self.advance();
                        end = self.pos;
                        Token::Walrus
                    } else {
                        Token::Colon
                    };

                    tokens.push(SpannedToken {
                        token: tok,
                        span: Span::new(start, end),
                    });

                    self.advance();
                }
                b'(' => {
                    tokens.push(SpannedToken {
                        token: Token::OParen,
                        span: Span::new(self.pos, self.pos),
                    });

                    self.advance();
                }
                b')' => {
                    tokens.push(SpannedToken {
                        token: Token::CParen,
                        span: Span::new(self.pos, self.pos),
                    });

                    self.advance();
                }
                b'<' => {
                    tokens.push(SpannedToken {
                        token: Token::OAngleBracket,
                        span: Span::new(self.pos, self.pos),
                    });

                    self.advance();
                }
                b'>' => {
                    tokens.push(SpannedToken {
                        token: Token::CAngleBracket,
                        span: Span::new(self.pos, self.pos),
                    });

                    self.advance();
                }
                b'[' => {
                    tokens.push(SpannedToken {
                        token: Token::OBracket,
                        span: Span::new(self.pos, self.pos),
                    });

                    self.advance();
                }
                b']' => {
                    tokens.push(SpannedToken {
                        token: Token::CBracket,
                        span: Span::new(self.pos, self.pos),
                    });

                    self.advance();
                }
                b'{' => {
                    tokens.push(SpannedToken {
                        token: Token::OCurlyBracket,
                        span: Span::new(self.pos, self.pos),
                    });

                    self.advance();
                }
                b'}' => {
                    tokens.push(SpannedToken {
                        token: Token::CCurlyBracket,
                        span: Span::new(self.pos, self.pos),
                    });

                    self.advance();
                }
                b',' => {
                    tokens.push(SpannedToken {
                        token: Token::Comma,
                        span: Span::new(self.pos, self.pos),
                    });

                    self.advance();
                }
                b'?' => {
                    tokens.push(SpannedToken {
                        token: Token::QuestionMark,
                        span: Span::new(self.pos, self.pos),
                    });

                    self.advance();
                }
                b'@' => {
                    // Allows for same behavior even in file with serialized data
                    // TODO: Should this be partially removed? Should @ be preserved?
                    if self.is_def_start() {
                        in_def = true;
                        self.pos += DEFINITION_SIZE;
                    } else if self.is_def_end() {
                        in_def = false;

                        tokens.push(SpannedToken {
                            token: Token::EOF,
                            // Needs - 1 because span is inclusive exclusive
                            // due to how bytes are seen
                            // Could be from bad decisions...
                            span: Span::new(self.pos, self.pos + DEFINITION_SIZE - 1),
                        });

                        // start_offset = self.pos + DEFINITION_SIZE;
                        break;
                    } else {
                        illegal_toks += 1;
                        tokens.push(self.recover_illegal(interner));
                    }
                }
                b'.' => {
                    let (start, end) = (self.pos, self.pos);

                    if self.peek_ahead(1) == b'.' && self.peek_ahead(2) == b'=' {
                        self.skip(2);

                        //TODO: Remove this concept
                        tokens.push(SpannedToken {
                            token: Token::DotRange,
                            span: Span::new(start, end),
                        });
                    } else {
                        tokens.push(SpannedToken {
                            token: Token::Dot,
                            span: Span::new(start, end),
                        });
                    }

                    self.advance();
                }
                b'#' => {
                    tokens.push(SpannedToken {
                        token: Token::HashSymbol,
                        span: Span::new(self.pos, self.pos),
                    });

                    self.advance();
                }
                b'|' => {
                    tokens.push(SpannedToken {
                        token: Token::VerticalBar,
                        span: Span::new(self.pos, self.pos),
                    });

                    self.advance();
                }
                b'"' => {
                    //FIX: Add clarity for -1. Maybe return the token itself.
                    self.advance();
                    tokens.push(self.read_quotes(interner));
                }
                b'-' => {
                    let (start, mut end) = (self.pos, self.pos);

                    let token = if self.peek_ahead(1) == b'>' {
                        self.advance();
                        end = self.pos;
                        Token::SlimArrow
                    } else {
                        Token::Hyphen
                    };

                    tokens.push(SpannedToken {
                        token,
                        span: Span::new(start, end),
                    });

                    self.advance();
                }
                b'=' => {
                    tokens.push(SpannedToken {
                        token: Token::Equals,
                        span: Span::new(self.pos, self.pos),
                    });

                    self.advance();
                }
                b'~' => {
                    tokens.push(SpannedToken {
                        token: Token::Tilde,
                        span: Span::new(self.pos, self.pos),
                    });

                    self.advance();
                }
                b'/' => {
                    if self.peek_ahead(1) == b'/' {
                        self.skip(2);
                        self.handle_comment();
                    } else if self.peek_ahead(1) == b'*' {
                        self.skip(2);
                        self.handle_multi_comment();
                    } else {
                        tokens.push(SpannedToken {
                            token: Token::Slash,
                            span: Span::new(self.pos, self.pos),
                        });

                        self.advance();
                    }
                }
                b'!' => {
                    tokens.push(SpannedToken {
                        token: Token::ExclamationPoint,
                        span: Span::new(self.pos, self.pos),
                    });

                    self.advance();
                }
                b'%' => {
                    tokens.push(SpannedToken {
                        token: Token::Percent,
                        span: Span::new(self.pos, self.pos),
                    });

                    self.advance();
                }
                _ => {
                    illegal_toks += 1;

                    tokens.push(self.recover_illegal(interner));
                    if illegal_toks > MAX_ILLEGAL_TOKS {
                        // TODO: Maybe this should be at the end because technically @ is illegal too
                        eprintln!("Maximum illegal tokens found.\nReporting then aborting...");
                        in_def = false;
                        // Should this just be done at the end of the loop by default?
                        tokens.push(SpannedToken {
                            token: Token::EOF,
                            span: Span::new(self.pos, self.pos),
                        });

                        break;
                    }
                }
            }
        }

        //WARN: This also isn't possible after the loader so may remove
        //Also odd handling
        if in_def && illegal_toks < MAX_ILLEGAL_TOKS {
            // Should abort
            eprintln!("Missing `@end`");
            panic!();
        }

        dbg!(&tokens);
        tokens
    }

    fn read_id(&mut self, interner: &mut Intern) -> SpannedToken {
        let mut id = Vec::with_capacity(8);

        let start = self.pos;
        dbg!(self.bytes[start] as char);

        //FIXME: Utf-8 compliance. Maybe.
        while self.pos < self.bytes.len() && self.peek().is_ascii_alphanumeric()
            || self.peek() == b'_'
        {
            let byte = self.advance();
            id.push(byte);
        }

        // Is one off since advance moves forward as a final step
        let end = self.pos - 1;

        let id = str::from_utf8(id.as_slice());

        match id {
            Ok(id) => {
                let id = interner.intern(id);

                SpannedToken {
                    token: Token::Id(id),
                    span: Span::new(start, end),
                }
            }
            Err(_) => {
                let response = "Invalid UTF-8. Could not parse id.";
                let resp_id = interner.intern(response);

                SpannedToken {
                    token: Token::Illegal(resp_id),
                    span: Span::new(start, end),
                }
            }
        }
    }

    fn read_num(&mut self, interner: &mut Intern) -> SpannedToken {
        let mut id = String::new();
        let start = self.pos;

        // TODO: Match specific handling for underscores for cleanliness.
        // Clean code, clean architecture, SOLID principles
        while self.pos < self.bytes.len() && self.peek().is_ascii_digit()
            || self.peek() == b'_'
            //TEST:
            || self.peek() == b'.'
        {
            let byte = self.advance();

            if byte == b'_' {
                continue;
            }

            id.push(byte as char);
        }

        //WARN:
        let end = self.pos - 1;

        let id = interner.intern(&id);

        SpannedToken {
            token: Token::Number(id),
            span: Span::new(start, end),
        }
    }

    //FIX: Currently fixing quote offset with - 1 but should likely just return start, end, tok
    fn read_quotes(&mut self, interner: &mut Intern) -> SpannedToken {
        let mut path: Vec<u8> = Vec::with_capacity(10);
        //WARN: THIS WAS CHANGED SEE IF IT BROKE
        let start = self.pos - 1;

        //FIXME: Could be more escapes
        //Should \0 be allowed?
        let escape_sequences = [b'n', b'r', b'\"', b'0', b'\\', b'x'];

        while self.pos < self.bytes.len() {
            match self.peek() {
                b'\\' => {
                    let a = self.advance();

                    path.push(a);

                    if !escape_sequences.contains(&self.peek()) {
                        return self.recover_illegal(interner);
                    }

                    let b = self.advance();

                    path.push(b);
                }
                b'\"' => {
                    self.advance();
                    break;
                }
                _ => path.push(self.advance()),
            }
        }

        //WARN: Compenstation for quotes being ignored
        let end = self.pos - 1;

        //TODO: Cleaner handle of failure to close string.
        //I believe the Java File file = new File(); detects this innately
        if self.pos == self.bytes.len() {
            let resp_id = interner.intern("Failed to close string literal and found <eof>");
            return SpannedToken {
                token: Token::Illegal(resp_id),
                span: Span::new(start, end),
            };
        }

        dbg!(&path);
        let path_res = str::from_utf8(path.as_slice());

        match path_res {
            Ok(p) => {
                let path_id = interner.intern(p);
                SpannedToken {
                    token: Token::Literal(path_id),
                    span: Span::new(start, end),
                }
            }
            Err(_) => {
                let response = "Invalid UTF-8. Could not parse literal.";
                let id = interner.intern(response);
                SpannedToken {
                    token: Token::Illegal(id),
                    span: Span::new(start, end),
                }
            }
        }
    }

    fn peek(&self) -> u8 {
        self.bytes.get(self.pos).copied().unwrap_or(b'\0')
    }

    fn is_def_start(&self) -> bool {
        if self.pos + 3 > self.bytes.len() {
            return false;
        }

        let possible_start = &self.bytes[self.pos..=self.pos + 3];

        if possible_start == "@def".as_bytes() {
            return true;
        }

        false
    }

    fn is_def_end(&mut self) -> bool {
        if self.pos + 3 > self.bytes.len() {
            return false;
        }

        let possible_end = &self.bytes[self.pos..=self.pos + 3];
        dbg!(str::from_utf8(&possible_end).unwrap());

        if possible_end == "@end".as_bytes() {
            return true;
        }

        false
    }

    //FIX: POSITION UTF-8 is off by one Why?
    fn recover_illegal(&mut self, interner: &mut Intern) -> SpannedToken {
        let start = self.pos;

        let mut err_str = String::new();

        while let Some(ch) = self.peek_char()
            && !ch.is_whitespace()
        {
            err_str.push(ch);

            println!("loop: id={}", &err_str);

            self.advance_char();
        }

        //FIX: Normal in comparison to read_id but still a bit concerning
        let end = self.pos - 1;

        println!("out: id={}", &err_str);

        let id = interner.intern(&err_str);

        dbg!(Span::new(start, end));

        SpannedToken {
            token: Token::Illegal(id),
            span: Span::new(start, end),
        }
    }

    fn peek_char(&mut self) -> Option<char> {
        let b = self.peek();

        if b <= 127 {
            return Some(b as char);
        }

        let end = std::cmp::min(self.pos + 3, self.bytes.len());

        let chunk = &self.bytes[self.pos..end];

        // // Lazy evaluation to avoid utf-8 checking entire self.bytes
        std::str::from_utf8(chunk)
            .ok()
            .and_then(|c| c.chars().next())
    }

    fn handle_comment(&mut self) {
        while self.peek() != b'\n' {
            self.advance();
        }
    }

    //TODO: Should this keep the depth even though the loader ensures this cant happen?
    fn handle_multi_comment(&mut self) {
        let mut depth = 1;
        // Avoiding recursion...
        while self.pos < self.bytes.len() && depth > 0 {
            dbg!(self.peek() as char, self.peek_ahead(1) as char);
            if self.peek() == b'/' && self.peek_ahead(1) == b'*' {
                self.skip(1);
                depth += 1;
            } else if self.peek() == b'*' && self.peek_ahead(1) == b'/' {
                self.skip(2);
                depth -= 1;
            } else {
                self.advance();
            }

            dbg!(self.peek() as char, self.peek_ahead(1) as char);
        }

        if depth > 0 {
            eprintln!("Could not find end of multi-line comment");
        }
    }

    // May return byte
    // WARN: This could be an issue. Many other places alike are present.
    fn skip(&mut self, dest: usize) {
        self.pos += dest;
    }

    fn peek_ahead(&mut self, dest: usize) -> u8 {
        self.bytes.get(self.pos + dest).copied().unwrap_or(b'\0')
    }

    fn advance(&mut self) -> u8 {
        let b = self.peek();
        self.pos += 1;
        b
    }

    fn advance_char(&mut self) -> Option<char> {
        let ch = self.peek_char();

        // Should this failing be a panic?
        self.pos += if let Some(c) = ch { c.len_utf8() } else { 1 };

        ch
    }

    //WARN: Compliant, but needs to ensure things don't break if utf-8 names can be used
    fn skip_whitespace(&mut self) {
        while self.peek().is_ascii_whitespace() {
            self.advance();
        }

        while let Some(ch) = self.peek_char()
            && ch.is_whitespace()
        {
            self.advance_char();
        }
    }
}
