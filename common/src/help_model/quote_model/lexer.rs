// Yes, this lexer looks odd.
use std::ops::Range;

use crate::{help_model::quote_model::token::TokenInfo, keywords};

use super::token::Token;

//TEST: This will all be removed after
const NEW_LN_SIG: f32 = 0.6;
const COMMENT_SIG: f32 = 0.10;
const ALPHA_NUM_SIG: f32 = 0.15;
const OTHER_SIG: f32 = 0.10;

#[derive(Debug)]
pub(super) struct Lexer<'a> {
    src_bytes: &'a [u8],
    pos: usize,
    end: usize,
    in_comment: bool,
    in_quote: bool,
    in_def: bool,
    q_type: char,
}

impl<'a> Lexer<'a> {
    pub(super) fn new(
        src_bytes: &'a [u8],
        search_range: &'a Range<usize>,
        q_type: char,
    ) -> Lexer<'a> {
        Lexer {
            src_bytes,
            pos: search_range.start,
            end: search_range.end,
            in_comment: false,
            in_quote: false,
            in_def: false,
            q_type,
        }
    }

    pub(super) fn tokenize(&mut self) -> Vec<TokenInfo> {
        let mut toks: Vec<TokenInfo> = Vec::new();

        loop {
            let ch = self.peek();

            if ch == '\0' || self.pos >= self.end {
                toks.push(TokenInfo {
                    tok: Token::EOF,
                    sig: 0.0,
                    in_comment: self.in_comment,
                });

                break;
            }

            match ch {
                '@' => {
                    if self.is_def_start() {
                        self.in_def = true;

                        toks.push(TokenInfo {
                            tok: Token::Def,
                            sig: 10.0,
                            in_comment: self.in_comment,
                        });

                        self.skip(keywords::DEFINITION_SIZE);
                    } else if self.is_def_end() {
                        self.in_def = false;

                        toks.push(TokenInfo {
                            tok: Token::End,
                            // Different type of signal
                            sig: 0.0,
                            in_comment: self.in_comment,
                        });

                        self.skip(keywords::DEFINITION_SIZE);
                    } else {
                        toks.push(TokenInfo {
                            tok: Token::Char('@'),
                            sig: OTHER_SIG,
                            in_comment: self.in_comment,
                        });

                        self.advance();
                    }
                }
                '\n' => {
                    toks.push(TokenInfo {
                        tok: Token::Char('\n'),
                        sig: NEW_LN_SIG,
                        in_comment: self.in_comment,
                    });

                    self.advance();
                }
                '/' => {
                    if self.peek_ahead(1) == b'/' {
                        self.in_comment = true;
                        self.handle_comment(&mut toks);
                        self.in_comment = false;
                    } else if self.peek_ahead(1) == b'*' {
                        self.in_comment = true;
                        self.handle_multi_comment(&mut toks);
                        self.in_comment = false;
                    } else {
                        toks.push(TokenInfo {
                            tok: Token::Char(ch),
                            sig: 0.3,
                            in_comment: self.in_comment,
                        });

                        self.advance();
                    }
                }
                ch if ch == self.q_type => {
                    let tok = self.advance_quote();

                    toks.push(TokenInfo {
                        tok,
                        sig: 0.0,
                        in_comment: self.in_comment,
                    });
                }
                ch if ch.is_alphanumeric() => {
                    toks.push(TokenInfo {
                        tok: Token::Char(ch),
                        sig: ALPHA_NUM_SIG,
                        in_comment: self.in_comment,
                    });

                    self.advance();
                }
                c => {
                    toks.push(TokenInfo {
                        tok: Token::Char(c),
                        sig: 0.3,
                        in_comment: self.in_comment,
                    });

                    self.advance();
                }
            }
        }

        toks
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

    fn handle_comment(&mut self, toks: &mut Vec<TokenInfo>) {
        while self.peek() != '\n' {
            if self.peek() == self.q_type {
                println!("Should be quote {}", self.peek());
                let tok = self.advance_quote();

                //FIX:
                toks.push(TokenInfo {
                    tok,
                    sig: 0.0,
                    in_comment: true,
                });
            } else {
                toks.push(TokenInfo {
                    tok: Token::Char(self.peek()),
                    sig: COMMENT_SIG,
                    in_comment: true,
                });
            }

            self.advance();
        }
    }

    fn handle_multi_comment(&mut self, toks: &mut Vec<TokenInfo>) {
        let mut depth = 1;
        // Avoiding recursion...
        // But why?
        while self.pos < self.src_bytes.len() && depth > 0 {
            if self.peek() == '/' && self.peek_ahead(1) == b'*' {
                toks.push(TokenInfo {
                    tok: Token::Char('/'),
                    sig: COMMENT_SIG,
                    in_comment: self.in_comment,
                });

                toks.push(TokenInfo {
                    tok: Token::Char('*'),
                    sig: COMMENT_SIG,
                    in_comment: self.in_comment,
                });

                self.skip(1);
                depth += 1;
            } else if self.peek() == '*' && self.peek_ahead(1) == b'/' {
                toks.push(TokenInfo {
                    tok: Token::Char('*'),
                    sig: COMMENT_SIG,
                    in_comment: self.in_comment,
                });

                toks.push(TokenInfo {
                    tok: Token::Char('/'),
                    sig: COMMENT_SIG,
                    in_comment: self.in_comment,
                });

                self.skip(2);
                depth -= 1;
            } else {
                if self.peek() == self.q_type {
                    let tok = self.advance_quote();
                    dbg!(&tok);

                    toks.push(TokenInfo {
                        tok,
                        sig: COMMENT_SIG,
                        in_comment: self.in_comment,
                    });
                } else {
                    toks.push(TokenInfo {
                        tok: Token::Char(self.peek()),
                        sig: COMMENT_SIG,
                        in_comment: self.in_comment,
                    });
                }

                self.advance();
            }
        }

        if depth > 0 {
            eprintln!("Could not find end of multi-line comment");
        }
    }

    fn peek(&mut self) -> char {
        let b = *self.src_bytes.get(self.pos).unwrap_or(&b'\0');

        if b <= 127 {
            return b as char;
        }

        let end = std::cmp::min(self.pos + 3, self.src_bytes.len());

        let chunk = &self.src_bytes[self.pos..end];

        std::str::from_utf8(chunk)
            .ok()
            .and_then(|c| c.chars().next())
            .unwrap_or('\0')
    }

    fn advance_quote(&mut self) -> Token {
        let tok = if self.in_quote {
            self.in_quote = false;
            Token::StrongEndQuote(self.pos)
        } else {
            self.in_quote = true;
            Token::StrongStartQuote(self.pos)
        };

        self.advance();

        tok
    }

    fn peek_ahead(&self, dest: usize) -> u8 {
        self.src_bytes
            .get(self.pos + dest)
            .copied()
            .unwrap_or(b'\0')
    }

    fn advance(&mut self) -> char {
        let ch = self.peek();
        self.pos += ch.len_utf8();
        ch
    }

    fn skip(&mut self, dest: usize) {
        self.pos += dest;
    }
}
