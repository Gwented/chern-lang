use std::fmt::Display;

use common::span::Span;

use crate::parser::ast::BinaryOp;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// This exists so that the interned value can be kept and displayed. It's also so a notation can be
// read within the lexer and stored without losing accuracy by setting it to something like i64
pub(crate) enum Notation {
    Bin = 2,
    Decimal = 10,
    Octal = 8,
    Hex = 16,
}

#[derive(Debug, Clone)]
pub struct SpannedToken {
    pub(crate) tok: Token,
    pub span: Span,
}

// WHAT
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Token {
    // To help with error messages
    // Keyword(Keyword),
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
    GreaterOrEq,
    LessOrEq,
    NotEq,
    Ampersand,
    And,
    Or,
    At,
    Caret,
    ExclamationPoint,
    Tilde,
    VerticalBar,
    Dot,
    Poison,
    EOF,
}

impl Token {
    pub(crate) fn kind(&self) -> TokenKind {
        match self {
            Token::Id(_) => TokenKind::Id,
            Token::Str(_) => TokenKind::Str,
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
            Token::GreaterOrEq => TokenKind::GreaterOrEq,
            Token::LessOrEq => TokenKind::LessOrEq,
            Token::NotEq => TokenKind::NotEq,
            Token::At => TokenKind::At,
            Token::And => TokenKind::And,
            Token::Or => TokenKind::Or,
            Token::Colon => TokenKind::Colon,
            Token::OParen => TokenKind::OParen,
            Token::CParen => TokenKind::CParen,
            Token::Plus => TokenKind::Plus,
            Token::Ampersand => TokenKind::Ampersand,
            Token::Hyphen => TokenKind::Hyphen,
            Token::ExclamationPoint => TokenKind::ExclamationPoint,
            Token::Asterisk => TokenKind::Asterisk,
            Token::Caret => TokenKind::Caret,
            Token::Tilde => TokenKind::Tilde,
            Token::Dot => TokenKind::Dot,
            Token::VerticalBar => TokenKind::VerticalBar,
            Token::Illegal(_) => TokenKind::Illegal,
            Token::EOF => TokenKind::EOF,
        }
    }

    // Um
    pub(crate) fn precedence(&self) -> Option<(BinaryOp, u8)> {
        match self {
            Token::Plus => Some((BinaryOp::Add, 1)),
            Token::Hyphen => Some((BinaryOp::Sub, 1)),
            Token::Asterisk => Some((BinaryOp::Mult, 2)),
            Token::Slash => Some((BinaryOp::Divide, 2)),
            Token::Percent => Some((BinaryOp::Mod, 2)),
            Token::OAngleBracket => Some((BinaryOp::Less, 3)),
            Token::CAngleBracket => Some((BinaryOp::Greater, 3)),
            Token::GreaterOrEq => Some((BinaryOp::GreaterOrEq, 3)),
            Token::LessOrEq => Some((BinaryOp::LessOrEq, 3)),
            Token::EqualTo => Some((BinaryOp::EqTo, 4)),
            Token::NotEq => Some((BinaryOp::NotEq, 4)),
            Token::And => Some((BinaryOp::And, 5)),
            Token::Or => Some((BinaryOp::Or, 6)),
            Token::Tilde => todo!(),
            Token::VerticalBar => todo!(),
            Token::Ampersand => todo!(),
            Token::Caret => todo!(),
            // Token::RightShift => todo!(),
            // Token::LeftShift => todo!(),
            _ => None,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub(crate) enum TokenKind {
    Id,
    Str,
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
    GreaterOrEq,
    LessOrEq,
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
    NotEq,
    OParen,
    CParen,
    Plus,
    Hyphen,
    Ampersand,
    At,
    And,
    Or,
    Caret,
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
            TokenKind::Str => write!(f, "string literal"),
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
            TokenKind::NotEq => write!(f, "!="),
            TokenKind::GreaterOrEq => write!(f, ">="),
            TokenKind::LessOrEq => write!(f, "<="),
            TokenKind::Hyphen => write!(f, "-"),
            TokenKind::At => write!(f, "@"),
            TokenKind::Or => write!(f, "||"),
            TokenKind::And => write!(f, "&&"),
            TokenKind::Ampersand => write!(f, "&"),
            TokenKind::ExclamationPoint => write!(f, "!"),
            TokenKind::Asterisk => write!(f, "*"),
            TokenKind::Walrus => write!(f, ":="),
            TokenKind::Tilde => write!(f, "~"),
            TokenKind::Dot => write!(f, "."),
            TokenKind::VerticalBar => write!(f, "|"),
            TokenKind::Illegal => write!(f, "illegal"),
            TokenKind::EOF => write!(f, "<eof>"),
            TokenKind::Poison => write!(f, "<poisoned>"),
            TokenKind::Caret => write!(f, "^"),
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
pub const AT: u64 = 1 << 27;
pub const NOT_EQ: u64 = 1 << 28;
pub const GREATER_OR_EQ: u64 = 1 << 29;
pub const LESS_OR_EQ: u64 = 1 << 30;
pub const EXCLAMATION_POINT: u64 = 1 << 31;
pub const TILDE: u64 = 1 << 32;
pub const DOT: u64 = 1 << 33;
pub const VERTICAL_BAR: u64 = 1 << 34;
pub const ILLEGAL: u64 = 1 << 35;
pub const OR: u64 = 1 << 36;
pub const AND: u64 = 1 << 37;
pub const AMPERSAND: u64 = 38;
pub const CARET: u64 = 39;
pub const POISON: u64 = 1 << 40;
pub const EOF: u64 = 1 << 41;

//FIX: PLEASE ASSERT THIS THING
impl TokenKind {
    pub(crate) fn to_u64(&self) -> u64 {
        // Ignore this...
        match self {
            TokenKind::Id => ID,
            TokenKind::Str => LITERAL,
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
            TokenKind::At => AT,
            TokenKind::GreaterOrEq => GREATER_OR_EQ,
            TokenKind::LessOrEq => LESS_OR_EQ,
            TokenKind::Or => OR,
            TokenKind::And => AND,
            TokenKind::NotEq => NOT_EQ,
            TokenKind::Caret => CARET,
            TokenKind::Ampersand => AMPERSAND,
            TokenKind::EOF => EOF,
        }
    }
}
