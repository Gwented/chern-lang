use common::{intern::Intern, symbols::Span};

use crate::token::{SpannedToken, Token};

pub struct Lexer<'a> {
    src_bytes: &'a [u8],
    pos: usize,
}

impl Lexer<'_> {
    pub fn new(src: &[u8], serial_start: usize) -> Lexer<'_> {
        Lexer {
            src_bytes: src,
            pos: serial_start,
        }
    }

    pub fn tokenize(&mut self) -> Vec<SpannedToken> {
        todo!()
    }

    fn read_id(&mut self, interner: &mut Intern) -> SpannedToken {
        let start = self.pos;

        while self.pos < self.src_bytes.len() && self.peek_char().is_alphanumeric()
            || self.peek() == b'_'
        {
            self.advance_char();
        }
        // Is one off since advance moves forward as a final step

        let end = self.pos;

        let id_str = str::from_utf8(&self.src_bytes[start..end])
            .expect("Cannot fail due to loop only accepting valid UTF-8 characters.");

        let id = interner.intern(&id_str);

        SpannedToken {
            tok: Token::Id(id),
            // Offset due to advance being done before leaving the loop.
            span: Span::new(start, end - 1),
        }
    }

    fn peek_char(&mut self) -> char {
        let b = self.peek();

        if b <= 127 {
            return b as char;
        }

        let end = std::cmp::min(self.pos + 3, self.src_bytes.len());

        let chunk = &self.src_bytes[self.pos..end];

        // Lazy evaluation to avoid utf-8 checking entire self.bytes
        std::str::from_utf8(chunk)
            .ok()
            .and_then(|c| c.chars().next())
            .unwrap_or('\0')
    }

    fn peek(&self) -> u8 {
        self.src_bytes.get(self.pos).copied().unwrap_or(b'\0')
    }

    fn peek_ahead(&mut self, dest: usize) -> u8 {
        self.src_bytes
            .get(self.pos + dest)
            .copied()
            .unwrap_or(b'\0')
    }

    fn skip(&mut self, dest: usize) {
        self.pos += dest;
    }

    fn handle_comment(&mut self) {
        while self.peek() != b'\n' {
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

    fn skip_whitespace(&mut self) {
        while self.peek().is_ascii_whitespace() {
            self.advance();
        }

        while self.peek_char().is_whitespace() {
            self.advance_char();
        }
    }
}
