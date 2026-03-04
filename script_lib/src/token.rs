use std::fmt::Display;

use common::symbols::{Cond, InnerArgs, Span, TypeIdent};

#[derive(Debug, Clone)]
pub struct SpannedToken {
    pub(crate) token: Token,
    pub(crate) span: Span,
}

// WARN: TEMP
#[derive(Debug, Clone, Copy)]
pub(crate) enum Token {
    Id(u32),
    Literal(u32),
    Number(u32),
    Illegal(u32),
    OParen,
    CParen,
    OBracket,
    CBracket,
    OCurlyBracket,
    CCurlyBracket,
    OAngleBracket,
    CAngleBracket,
    QuestionMark,
    Equals,
    Colon,
    // This name NEEDS to be changed
    Walrus,
    Comma,
    SlimArrow,
    DotRange,
    Slash,
    HashSymbol,
    Percent,
    Hyphen,
    // At,
    ExclamationPoint,
    Asterisk,
    DoubleQuotes,
    Tilde,
    Dot,
    VerticalBar,
    Poison,
    EOF,
}

impl Token {
    pub fn kind(&self) -> TokenKind {
        match self {
            Token::Id(_) => TokenKind::Id,
            Token::Literal(_) => TokenKind::Literal,
            Token::Number(_) => TokenKind::Number,
            Token::OBracket => TokenKind::OBracket,
            Token::CBracket => TokenKind::CBracket,
            Token::OCurlyBracket => TokenKind::OCurlyBracket,
            Token::CCurlyBracket => TokenKind::CCurlyBracket,
            Token::QuestionMark => TokenKind::QuestionMark,
            Token::Equals => TokenKind::Equals,
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
            Token::Hyphen => TokenKind::Hyphen,
            Token::ExclamationPoint => TokenKind::ExclamationPoint,
            Token::Asterisk => TokenKind::Asterisk,
            Token::DoubleQuotes => TokenKind::DoubleQuotes,
            Token::Tilde => TokenKind::Tilde,
            Token::Dot => TokenKind::Dot,
            Token::VerticalBar => TokenKind::VerticalBar,
            Token::Illegal(_) => TokenKind::Illegal,
            Token::EOF => TokenKind::EOF,
        }
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub(crate) enum TokenKind {
    Id,
    Literal,
    // START OF TYPES
    // I8,
    // U8,
    // I16,
    // U16,
    // F16,
    // I32,
    // U32,
    // F32,
    // I64,
    // U64,
    // F64,
    // I128,
    // U128,
    // F128,
    // Sized,
    // Unsized,
    // Str,
    // BigInt,
    // BigFloat,
    // List,
    // Set,
    // Map,
    // // QUESTIONABLE
    // Any,
    // //
    // Type,
    // UserType,
    // END OF TYPES
    Number,
    OBracket,
    CBracket,
    OCurlyBracket,
    CCurlyBracket,
    QuestionMark,
    Equals,
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
    Hyphen,
    // At,
    ExclamationPoint,
    Asterisk,
    DoubleQuotes,
    Tilde,
    Dot,
    VerticalBar,
    //TODO: Include branch or specific state of lexer. Maybe.
    Illegal,
    Poison,
    EOF,
}

impl Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenKind::Id => write!(f, "identifier"),
            TokenKind::Literal => write!(f, "literal"),
            TokenKind::Number => write!(f, "number"),
            TokenKind::OBracket => write!(f, "["),
            TokenKind::CBracket => write!(f, "]"),
            TokenKind::OCurlyBracket => write!(f, "{{"),
            TokenKind::CCurlyBracket => write!(f, "}}"),
            TokenKind::QuestionMark => write!(f, "?"),
            TokenKind::Equals => write!(f, "="),
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
            TokenKind::Hyphen => write!(f, "-"),
            TokenKind::ExclamationPoint => write!(f, "!"),
            TokenKind::Asterisk => write!(f, "*"),
            TokenKind::Walrus => write!(f, ":="),
            TokenKind::DoubleQuotes => write!(f, "\""),
            TokenKind::Tilde => write!(f, "~"),
            TokenKind::Dot => write!(f, "."),
            TokenKind::VerticalBar => write!(f, "|"),
            TokenKind::Illegal => write!(f, "illegal"),
            TokenKind::EOF => write!(f, "<eof>"),
            // TokenKind::I8 => write!(f, "i8"),
            // TokenKind::U8 => write!(f, "u8"),
            // TokenKind::I16 => write!(f, "i16"),
            // TokenKind::U16 => write!(f, "u16"),
            // TokenKind::F16 => write!(f, "f16"),
            // TokenKind::I32 => write!(f, "i32"),
            // TokenKind::U32 => write!(f, "u32"),
            // TokenKind::F32 => write!(f, "f32"),
            // TokenKind::I64 => write!(f, "i64"),
            // TokenKind::U64 => write!(f, "u64"),
            // TokenKind::F64 => write!(f, "f64"),
            // TokenKind::I128 => write!(f, "i128"),
            // TokenKind::U128 => write!(f, "u128"),
            // TokenKind::F128 => write!(f, "f128"),
            // TokenKind::Sized => write!(f, "sized"),
            // TokenKind::Unsized => write!(f, "unsized"),
            // TokenKind::Str => write!(f, "str"),
            // TokenKind::BigInt => write!(f, "BigInt"),
            // TokenKind::BigFloat => write!(f, "BigFloat"),
            // TokenKind::Type => write!(f, "type"),
            // TokenKind::List => write!(f, "List"),
            // TokenKind::Set => write!(f, "Set"),
            // TokenKind::Map => write!(f, "Map"),
            // TokenKind::Any => write!(f, "Any"),
            // TokenKind::UserType => write!(f, "User type"),
            TokenKind::Poison => write!(f, "<poisoned>"),
        }
    }
}

// IS THIS EVEN OPTIMAL?
const ID: u64 = 1 << 0;
const LITERAL: u64 = 1 << 1;
const NUMBER: u64 = 1 << 2;
const O_BRACKET: u64 = 1 << 3;
const C_BRACKET: u64 = 1 << 4;
const O_CURLY_BRACKET: u64 = 1 << 5;
const C_CURLY_BRACKET: u64 = 1 << 6;
const QUESTION_MARK: u64 = 1 << 7;
const EQUALS: u64 = 1 << 8;
const WALRUS: u64 = 1 << 9;
const O_ANGLE_BRACKET: u64 = 1 << 10;
const C_ANGLE_BRACKET: u64 = 1 << 11;
const COMMA: u64 = 1 << 12;
const SLIM_ARROW: u64 = 1 << 13;
const SLASH: u64 = 1 << 14;
const HASH_SYMBOL: u64 = 1 << 15;
const DOT_RANGE: u64 = 1 << 16;
const PERCENT: u64 = 1 << 17;
const COLON: u64 = 1 << 18;
const O_PAREN: u64 = 1 << 19;
const C_PAREN: u64 = 1 << 20;
const HYPHEN: u64 = 1 << 21;
const EXCLAMATION_POINT: u64 = 1 << 22;
const ASTERISK: u64 = 1 << 23;
const DOUBLE_QUOTES: u64 = 1 << 24;
const TILDE: u64 = 1 << 25;
const DOT: u64 = 1 << 26;
const VERTICAL_BAR: u64 = 1 << 27;
const ILLEGAL: u64 = 1 << 28;
const POISON: u64 = 1 << 29;
const EOF: u64 = 1 << 30;

impl TokenKind {
    pub(crate) fn to_u64(&self) -> u64 {
        // Ignore this...
        match self {
            TokenKind::Id => ID,
            TokenKind::Literal => LITERAL,
            TokenKind::Number => NUMBER,
            TokenKind::OBracket => O_BRACKET,
            TokenKind::CBracket => C_BRACKET,
            TokenKind::OCurlyBracket => O_CURLY_BRACKET,
            TokenKind::CCurlyBracket => C_CURLY_BRACKET,
            TokenKind::QuestionMark => QUESTION_MARK,
            TokenKind::Equals => EQUALS,
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
            TokenKind::Hyphen => HYPHEN,
            TokenKind::ExclamationPoint => EXCLAMATION_POINT,
            TokenKind::Asterisk => ASTERISK,
            TokenKind::DoubleQuotes => DOUBLE_QUOTES,
            TokenKind::Tilde => TILDE,
            TokenKind::Dot => DOT,
            TokenKind::VerticalBar => VERTICAL_BAR,
            TokenKind::Illegal => ILLEGAL,
            TokenKind::Poison => POISON,
            TokenKind::EOF => EOF,
        }
    }
}

//FIX: Move this
#[derive(Debug)]
pub(crate) enum ActualPrimitives {
    I8,
    U8,
    I16,
    U16,
    F16,
    I32,
    U32,
    F32,
    I64,
    U64,
    F64,
    I128,
    U128,
    F128,
    Sized,
    Unsized,
    Bool,
    Nil,
    Char,
    Str,
    BigInt,
    BigFloat,
    // Template
    // TypeDef
    // ActualType
    List(TypeIdent),
    Set(TypeIdent),
    // ActualType
    Map(TypeIdent, TypeIdent),
    // Activation from None
    Any(Option<TypeIdent>),
}

impl ActualPrimitives {
    pub(crate) fn kind(&self) -> ActualTypeKind {
        match self {
            ActualPrimitives::I8 => ActualTypeKind::I8,
            ActualPrimitives::U8 => ActualTypeKind::U8,
            ActualPrimitives::I16 => ActualTypeKind::I16,
            ActualPrimitives::U16 => ActualTypeKind::U16,
            ActualPrimitives::F16 => ActualTypeKind::F16,
            ActualPrimitives::I32 => ActualTypeKind::I32,
            ActualPrimitives::U32 => ActualTypeKind::U32,
            ActualPrimitives::F32 => ActualTypeKind::F32,
            ActualPrimitives::I64 => ActualTypeKind::I64,
            ActualPrimitives::U64 => ActualTypeKind::U64,
            ActualPrimitives::F64 => ActualTypeKind::F64,
            ActualPrimitives::I128 => ActualTypeKind::I128,
            ActualPrimitives::U128 => ActualTypeKind::U128,
            ActualPrimitives::F128 => ActualTypeKind::F128,
            ActualPrimitives::Sized => ActualTypeKind::Sized,
            ActualPrimitives::Unsized => ActualTypeKind::Unsized,
            ActualPrimitives::Bool => ActualTypeKind::Bool,
            ActualPrimitives::Nil => ActualTypeKind::Nil,
            ActualPrimitives::Char => ActualTypeKind::Char,
            ActualPrimitives::Str => ActualTypeKind::Str,
            ActualPrimitives::BigInt => ActualTypeKind::BigInt,
            ActualPrimitives::BigFloat => ActualTypeKind::BigFloat,
            // ActualPrimitives::Template(template) => ActualTypeKind::Template,
            // ActualPrimitives::Definition(type_def) => ActualTypeKind::TypeDef,
            ActualPrimitives::List(_) => ActualTypeKind::List,
            ActualPrimitives::Set(_) => ActualTypeKind::Set,
            ActualPrimitives::Map(_, _) => ActualTypeKind::Map,
            ActualPrimitives::Any(_) => ActualTypeKind::Any,
        }
    }
}

pub(crate) enum ActualTypeKind {
    I8,
    U8,
    I16,
    U16,
    F16,
    I32,
    U32,
    F32,
    I64,
    U64,
    F64,
    I128,
    U128,
    F128,
    Sized,
    Unsized,
    Str,
    Char,
    Nil,
    Bool,
    BigInt,
    BigFloat,
    List,
    Set,
    Map,
    Any,
    TypeDef,
    Template,
}

//TEST:
impl Display for ActualTypeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActualTypeKind::I8 => write!(f, "i8"),
            ActualTypeKind::U8 => write!(f, "u8"),
            ActualTypeKind::I16 => write!(f, "u16"),
            ActualTypeKind::U16 => write!(f, "u16"),
            ActualTypeKind::F16 => write!(f, "f16"),
            ActualTypeKind::I32 => write!(f, "i32"),
            ActualTypeKind::U32 => write!(f, "u32"),
            ActualTypeKind::F32 => write!(f, "f32"),
            ActualTypeKind::I64 => write!(f, "i64"),
            ActualTypeKind::U64 => write!(f, "u64"),
            ActualTypeKind::F64 => write!(f, "f64"),
            ActualTypeKind::I128 => write!(f, "i128"),
            ActualTypeKind::U128 => write!(f, "u128"),
            ActualTypeKind::F128 => write!(f, "f128"),
            ActualTypeKind::Sized => write!(f, "sized"),
            ActualTypeKind::Unsized => write!(f, "unsized"),
            ActualTypeKind::Str => write!(f, "str"),
            ActualTypeKind::Char => write!(f, "char"),
            ActualTypeKind::Nil => write!(f, "nil"),
            ActualTypeKind::Bool => write!(f, "bool"),
            ActualTypeKind::BigInt => write!(f, "BigInt"),
            ActualTypeKind::BigFloat => write!(f, "BigFloat"),
            ActualTypeKind::List => write!(f, "List"),
            ActualTypeKind::Set => write!(f, "Set"),
            ActualTypeKind::Map => write!(f, "Map"),
            ActualTypeKind::Any => write!(f, "Any"),
            ActualTypeKind::TypeDef => write!(f, "TypeDef"),
            ActualTypeKind::Template => write!(f, "Template"),
        }
    }
}

// The weight of every enum grows heavy, I don't know what isn't an enum anymore.
// Just one more enum.

#[derive(Debug)]
pub struct Template {
    // Should this be a symbol or type id?
    pub(crate) type_id: TypeIdent,
    pub(crate) args: Vec<InnerArgs>,
    // May remove conditions
    pub(crate) conds: Vec<Cond>,
    // Fields can be variants or separate strugg <-- Sgwom
    //WARN:
    pub(crate) fields: Vec<TypeIdent>,
    pub(crate) repre: Repre, //TODO: Typed ids please
                             //No
}

#[derive(Debug)]
pub(crate) enum Repre {
    Struct,
    Enum,
}

impl Template {
    pub(crate) fn new(type_id: TypeIdent, repre: Repre) -> Template {
        Template {
            type_id,
            args: Vec::new(),
            conds: Vec::new(),
            fields: Vec::new(),
            repre,
        }
    }
}

//WARN: May not need to create
// #[derive(Debug)]
// pub struct EnumTemplate {
//     pub(crate) id: SymbolId,
//     pub(crate) args: Vec<InnerArgs>,
//     pub(crate) conds: Vec<Cond>,
//     pub(crate) variants: Vec<SymbolId>,
// }

//FIXME:
// No
// PLEASE change this from a try_from
// Maybe
impl TryFrom<u32> for ActualPrimitives {
    type Error = ();

    fn try_from(v: u32) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(ActualPrimitives::I8),
            1 => Ok(ActualPrimitives::U8),
            2 => Ok(ActualPrimitives::I16),
            3 => Ok(ActualPrimitives::U16),
            4 => Ok(ActualPrimitives::F16),
            5 => Ok(ActualPrimitives::I32),
            6 => Ok(ActualPrimitives::U32),
            7 => Ok(ActualPrimitives::F32),
            8 => Ok(ActualPrimitives::I64),
            9 => Ok(ActualPrimitives::U64),
            10 => Ok(ActualPrimitives::F64),
            11 => Ok(ActualPrimitives::I128),
            12 => Ok(ActualPrimitives::U128),
            13 => Ok(ActualPrimitives::F128),
            14 => Ok(ActualPrimitives::Sized),
            15 => Ok(ActualPrimitives::Unsized),
            16 => Ok(ActualPrimitives::Char),
            17 => Ok(ActualPrimitives::Str),
            18 => Ok(ActualPrimitives::Bool),
            19 => Ok(ActualPrimitives::Nil),
            20 => Ok(ActualPrimitives::BigInt),
            21 => Ok(ActualPrimitives::BigFloat),
            // 25 => Ok(ReservedKeyword::Bind),
            // 26 => Ok(ReservedKeyword::Var),
            // 27 => Ok(ReservedKeyword::Nest),
            // 28 => Ok(ReservedKeyword::ComplexRules),
            _ => Err(()),
        }
    }
}
