// May or may not use same tokens as script

use common::span::Span;

#[repr(u8)]
#[derive(Debug, Clone, Copy)]
// This exists so that the interned value can be kept and displayed. It's also so a notation can be
// read within the lexer and stored without losing accuracy by setting it to something like i64
pub(crate) enum Notation {
    Bin = 2,
    Decimal = 10,
    Octal = 8,
    Hex = 16,
}

pub struct SpannedToken {
    pub(crate) tok: Token,
    pub span: Span,
}

#[derive(Debug, Clone, Copy)]
pub enum Token {
    Id(u32),
    Str(u32),
    Integer(u32, Notation),
    Float(u32, Notation),
    Illegal(u32),
    Char(char),
    OParen,
    CParen,
    OBracket,
    CBracket,
    OCurlyBracket,
    CCurlyBracket,
    OAngleBracket,
    CAngleBracket,
    Colon,
    Comma,
    SlimArrow,
    Slash,
    HashSymbol,
    Asterisk,
    Hyphen,
    ExclamationPoint,
    Tilde,
    Dot,
    Poison,
    EOF,
}
