use crate::symbols::Span;

#[derive(Debug, Clone, Copy)]
pub(super) enum Token {
    Def,
    StartQuote(usize),
    EndQuote(usize),
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
