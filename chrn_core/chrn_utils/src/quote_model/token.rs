#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Token {
    Def,
    StrongStartQuote(usize),
    StrongEndQuote(usize),
    Char(char),
    End,
    EOF,
}

#[derive(Debug, Clone)]
pub(super) struct TokenInfo {
    pub(super) tok: Token,
    pub(super) sig: f32,
    pub(super) in_comment: bool,
}
