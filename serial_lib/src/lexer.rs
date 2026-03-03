pub(crate) struct Lexer<'a> {
    text: &'a [u8],
    pos: usize,
}

impl Lexer<'_> {
    pub fn new(text: &[u8], lex_offset: usize) -> Lexer<'_> {
        Lexer {
            text,
            pos: 0 + lex_offset,
        }
    }
}
