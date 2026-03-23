pub struct Lexer<'a> {
    src: &'a [u8],
    pos: usize,
}

impl Lexer<'_> {
    pub fn new(src: &[u8], serial_start: usize) -> Lexer<'_> {
        Lexer {
            src,
            pos: 0 + serial_start,
        }
    }

    pub fn tokenize(&mut self) /*-> Vec<Token>*/
    {
        todo!()
    }
}
