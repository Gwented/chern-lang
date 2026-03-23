use std::fmt::Display;

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

#[derive(Debug, Clone, Copy)]
pub(crate) enum Token {
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
    QuestionMark,
    Assign,
    EqualTo,
    Colon,
    // This name NEEDS to be changed
    Walrus,
    Comma,
    SlimArrow,
    DotRange,
    Slash,
    HashSymbol,
    Percent,
    Plus,
    Asterisk,
    Hyphen,
    // At,
    ExclamationPoint,
    Tilde,
    VerticalBar,
    Dot,
    Poison,
    EOF,
}

impl Token {
    pub fn kind(&self) -> TokenKind {
        match self {
            Token::Id(_) => TokenKind::Id,
            Token::Str(_) => TokenKind::Literal,
            Token::Integer(_, _) => TokenKind::Integer,
            Token::Float(_, _) => TokenKind::Float,
            Token::Char(_) => TokenKind::Char,
            Token::OBracket => TokenKind::OBracket,
            Token::CBracket => TokenKind::CBracket,
            Token::OCurlyBracket => TokenKind::OCurlyBracket,
            Token::CCurlyBracket => TokenKind::CCurlyBracket,
            Token::QuestionMark => TokenKind::QuestionMark,
            Token::Assign => TokenKind::Assign,
            Token::EqualTo => TokenKind::EqualTo,
            Token::Poison => TokenKind::Poison,
            Token::Walrus => TokenKind::Walrus,
            Token::OAngleBracket => TokenKind::OAngleBracket,
            Token::CAngleBracket => TokenKind::CAngleBracket,
            Token::Comma => TokenKind::Comma,
            Token::SlimArrow => TokenKind::SlimArrow,
            Token::DotRange => TokenKind::DotRange,
            Token::Slash => TokenKind::Slash,
            Token::HashSymbol => TokenKind::HashSymbol,
            Token::Percent => TokenKind::Percent,
            Token::Colon => TokenKind::Colon,
            Token::OParen => TokenKind::OParen,
            Token::CParen => TokenKind::CParen,
            Token::Plus => TokenKind::Plus,
            Token::Hyphen => TokenKind::Hyphen,
            Token::ExclamationPoint => TokenKind::ExclamationPoint,
            Token::Asterisk => TokenKind::Asterisk,
            Token::Tilde => TokenKind::Tilde,
            Token::Dot => TokenKind::Dot,
            Token::VerticalBar => TokenKind::VerticalBar,
            Token::Illegal(_) => TokenKind::Illegal,
            Token::EOF => TokenKind::EOF,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum TokenKind {
    Id,
    Literal,
    Integer,
    Float,
    Char,
    OBracket,
    CBracket,
    OCurlyBracket,
    CCurlyBracket,
    QuestionMark,
    Assign,
    EqualTo,
    Walrus,
    OAngleBracket,
    CAngleBracket,
    Comma,
    SlimArrow,
    Slash,
    HashSymbol,
    DotRange,
    Percent,
    Colon,
    OParen,
    CParen,
    Plus,
    Hyphen,
    // At,
    ExclamationPoint,
    Asterisk,
    Tilde,
    Dot,
    VerticalBar,
    Illegal,
    Poison,
    EOF,
}

impl Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenKind::Id => write!(f, "identifier"),
            TokenKind::Literal => write!(f, "string literal"),
            TokenKind::Integer => write!(f, "integer"),
            TokenKind::Float => write!(f, "float"),
            TokenKind::Char => write!(f, "char"),
            TokenKind::OBracket => write!(f, "["),
            TokenKind::CBracket => write!(f, "]"),
            TokenKind::OCurlyBracket => write!(f, "{{"),
            TokenKind::CCurlyBracket => write!(f, "}}"),
            TokenKind::QuestionMark => write!(f, "?"),
            TokenKind::Assign => write!(f, "="),
            TokenKind::EqualTo => write!(f, "=="),
            TokenKind::OAngleBracket => write!(f, "<"),
            TokenKind::CAngleBracket => write!(f, ">"),
            TokenKind::Comma => write!(f, ","),
            TokenKind::SlimArrow => write!(f, "->"),
            TokenKind::DotRange => write!(f, "(range)"),
            TokenKind::Slash => write!(f, "/"),
            TokenKind::HashSymbol => write!(f, "#"),
            TokenKind::Percent => write!(f, "%"),
            TokenKind::Colon => write!(f, ":"),
            TokenKind::OParen => write!(f, "("),
            TokenKind::CParen => write!(f, ")"),
            TokenKind::Plus => write!(f, "+"),
            TokenKind::Hyphen => write!(f, "-"),
            TokenKind::ExclamationPoint => write!(f, "!"),
            TokenKind::Asterisk => write!(f, "*"),
            TokenKind::Walrus => write!(f, ":="),
            TokenKind::Tilde => write!(f, "~"),
            TokenKind::Dot => write!(f, "."),
            TokenKind::VerticalBar => write!(f, "|"),
            TokenKind::Illegal => write!(f, "illegal"),
            TokenKind::EOF => write!(f, "<eof>"),
            TokenKind::Poison => write!(f, "<poisoned>"),
        }
    }
}

// Please assert this
// We don't need assertions.
// Please.
pub const ID: u64 = 1 << 0;
pub const LITERAL: u64 = 1 << 1;
pub const INTEGER: u64 = 1 << 2;
pub const FLOAT: u64 = 1 << 3;
pub const CHAR: u64 = 1 << 4;
pub const O_BRACKET: u64 = 1 << 5;
pub const C_BRACKET: u64 = 1 << 6;
pub const O_CURLY_BRACKET: u64 = 1 << 7;
pub const C_CURLY_BRACKET: u64 = 1 << 8;
pub const QUESTION_MARK: u64 = 1 << 9;
pub const ASSIGN: u64 = 1 << 10;
pub const EQUAL_TO: u64 = 1 << 11;
pub const WALRUS: u64 = 1 << 12;
pub const O_ANGLE_BRACKET: u64 = 1 << 13;
pub const C_ANGLE_BRACKET: u64 = 1 << 14;
pub const COMMA: u64 = 1 << 15;
pub const SLIM_ARROW: u64 = 1 << 16;
pub const SLASH: u64 = 1 << 17;
pub const HASH_SYMBOL: u64 = 1 << 18;
pub const DOT_RANGE: u64 = 1 << 19;
pub const PERCENT: u64 = 1 << 20;
pub const COLON: u64 = 1 << 21;
pub const O_PAREN: u64 = 1 << 22;
pub const C_PAREN: u64 = 1 << 23;
pub const PLUS: u64 = 1 << 24;
pub const HYPHEN: u64 = 1 << 25;
pub const ASTERISK: u64 = 1 << 26;
pub const EXCLAMATION_POINT: u64 = 1 << 27;
pub const TILDE: u64 = 1 << 28;
pub const DOT: u64 = 1 << 29;
pub const VERTICAL_BAR: u64 = 1 << 30;
pub const ILLEGAL: u64 = 1 << 31;
pub const POISON: u64 = 1 << 32;
pub const EOF: u64 = 1 << 33;

//FIX: PLEASE ASSERT THIS THING
impl TokenKind {
    pub fn to_u64(&self) -> u64 {
        // Ignore this...
        match self {
            TokenKind::Id => ID,
            TokenKind::Literal => LITERAL,
            TokenKind::Integer => INTEGER,
            TokenKind::Float => FLOAT,
            TokenKind::Char => CHAR,
            TokenKind::OBracket => O_BRACKET,
            TokenKind::CBracket => C_BRACKET,
            TokenKind::OCurlyBracket => O_CURLY_BRACKET,
            TokenKind::CCurlyBracket => C_CURLY_BRACKET,
            TokenKind::QuestionMark => QUESTION_MARK,
            TokenKind::Assign => ASSIGN,
            TokenKind::EqualTo => EQUAL_TO,
            TokenKind::Walrus => WALRUS,
            TokenKind::OAngleBracket => O_ANGLE_BRACKET,
            TokenKind::CAngleBracket => C_ANGLE_BRACKET,
            TokenKind::Comma => COMMA,
            TokenKind::SlimArrow => SLIM_ARROW,
            TokenKind::Slash => SLASH,
            TokenKind::HashSymbol => HASH_SYMBOL,
            TokenKind::DotRange => DOT_RANGE,
            TokenKind::Percent => PERCENT,
            TokenKind::Colon => COLON,
            TokenKind::OParen => O_PAREN,
            TokenKind::CParen => C_PAREN,
            TokenKind::Plus => PLUS,
            TokenKind::Hyphen => HYPHEN,
            TokenKind::ExclamationPoint => EXCLAMATION_POINT,
            TokenKind::Asterisk => ASTERISK,
            TokenKind::Tilde => TILDE,
            TokenKind::Dot => DOT,
            TokenKind::VerticalBar => VERTICAL_BAR,
            TokenKind::Illegal => ILLEGAL,
            TokenKind::Poison => POISON,
            TokenKind::EOF => EOF,
        }
    }
}

//FIXME:
// No
// PLEASE change this from a try_from
// Maybe
// Definitely
