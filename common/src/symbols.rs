use std::fmt::Display;

use crate::{builtins::BuiltinType, keywords::Keyword};

#[derive(Debug, Clone, Copy)]
pub enum TypedId {
    Struct(StructId),
    Enum(EnumId),
    TypeDef(TypeDefId),
    Func(FuncId),
    BuiltinType(BuiltinTypeId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId {
    pub id: u32,
}

impl SymbolId {
    pub fn new(id: u32) -> SymbolId {
        SymbolId { id }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AstId {
    pub id: u32,
}

impl AstId {
    pub fn new(id: u32) -> AstId {
        AstId { id }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NameId {
    pub id: u32,
}

impl NameId {
    pub fn new(id: u32) -> NameId {
        NameId { id }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuncId {
    pub id: u32,
}

impl FuncId {
    pub fn new(id: u32) -> FuncId {
        FuncId { id }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnumId {
    pub id: u32,
}

impl EnumId {
    pub fn new(id: u32) -> EnumId {
        EnumId { id }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructId {
    pub id: u32,
}

impl StructId {
    pub fn new(id: u32) -> StructId {
        StructId { id }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinTypeId {
    pub id: u32,
}

impl BuiltinTypeId {
    pub fn new(id: u32) -> BuiltinTypeId {
        BuiltinTypeId { id }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeDefId {
    pub id: u32,
}

impl TypeDefId {
    pub fn new(id: u32) -> TypeDefId {
        TypeDefId { id }
    }
}

//TODO: Should maybe be somewhere else but fine for now
//Could this be u32?
#[derive(Debug, Clone)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Span {
        Span { start, end }
    }
}

pub struct SpannedNameId {
    pub name_id: NameId,
    pub span: Span,
}

impl SpannedNameId {
    pub fn new(name_id: NameId, span: Span) -> SpannedNameId {
        SpannedNameId { name_id, span }
    }
}

pub struct SpannedBuiltinType {
    pub ty: BuiltinType,
    pub span: Span,
}

impl SpannedBuiltinType {
    pub fn new(ty: BuiltinType, span: Span) -> SpannedBuiltinType {
        SpannedBuiltinType { ty, span }
    }
}

#[derive(Debug, Clone)]
pub enum Cond {
    //FIX:
    Func(FuncId),
    // Maybe this shouldn't be a function
    IsEmpty,
    IsWhitespace,
    // Probably should just attach bool
    // should likely be removed
    Not(Box<Cond>),
}

// I'm actually fine with this.
impl Cond {
    /// Only returns a condition if it is solely a keyword, and excludes conditions such as
    /// `Contains()`
    // This is really really really really smelly
    pub fn try_from_id(id: u32) -> Option<Cond> {
        match Keyword::try_as_kw(id) {
            Some(kw) => match kw {
                Keyword::IsEmpty => Some(Cond::IsEmpty),
                Keyword::IsWhitespace => Some(Cond::IsWhitespace),
                _ => None,
            },
            None => None,
        }
    }

    pub fn try_from_kw(kw: Keyword) -> Option<Cond> {
        match kw {
            Keyword::IsEmpty => Some(Cond::IsEmpty),
            Keyword::IsWhitespace => Some(Cond::IsWhitespace),
            _ => None,
        }
    }
}

//TEST:
// public static void main(String[] args) { for (int i = 0; i < args.length; ++i) {
// System.out.printf("%d: %s", i, args[i]) } }

//NOTE: If a new argument is added ensure this is updated
pub static ARGS_ARRAY: [&str; 5] = ["warn", "scient", "hex", "bin", "octal"];

#[derive(Debug, Clone)]
pub struct SpannedInnerArgs {
    pub inner_arg: InnerArgs,
    pub span: Span,
}

impl SpannedInnerArgs {
    pub fn new(inner_arg: InnerArgs, span: Span) -> SpannedInnerArgs {
        SpannedInnerArgs { inner_arg, span }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InnerArgs {
    Warn,
    Scientific,
    Hex,
    Binary,
    Octal,
}

impl InnerArgs {
    //TEST:
    /// This MUST be used after ensuring the type is a primitive, not a data structure.
    // Maybe this is a good time to use kind
    pub fn supports_builtin_type(&self, builtin_type: &BuiltinType) -> bool {
        match self {
            InnerArgs::Warn => true,
            InnerArgs::Scientific | InnerArgs::Hex | InnerArgs::Binary | InnerArgs::Octal => {
                match builtin_type {
                    BuiltinType::I8
                    | BuiltinType::U8
                    | BuiltinType::I16
                    | BuiltinType::U16
                    | BuiltinType::F16
                    | BuiltinType::I32
                    | BuiltinType::U32
                    | BuiltinType::F32
                    | BuiltinType::I64
                    | BuiltinType::U64
                    | BuiltinType::F64
                    | BuiltinType::I128
                    | BuiltinType::U128
                    | BuiltinType::F128
                    | BuiltinType::Sized
                    | BuiltinType::BigInt
                    | BuiltinType::BigFloat
                    | BuiltinType::Unsized
                    //NOTE: Checks this at runtime
                    |BuiltinType::Any(_) => true,
                    // Maybe make this unreachable and depending on the caller handling the inner
                    BuiltinType::List(_)
                    |BuiltinType::Set(_)
                    |BuiltinType::Map(_, _) => unreachable!("TypeResolver broke"),
                    _ => false,
                }
            }
        }
    }
}

impl Display for InnerArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            InnerArgs::Warn => write!(f, "warn"),
            InnerArgs::Scientific => write!(f, "scient"),
            InnerArgs::Hex => write!(f, "hex"),
            InnerArgs::Binary => write!(f, "bin"),
            InnerArgs::Octal => write!(f, "octal"),
        }
    }
}

//TODO: Should be some or none
impl<'a> TryFrom<&'a str> for InnerArgs {
    type Error = &'a str;

    fn try_from(v: &'a str) -> Result<Self, Self::Error> {
        match v {
            "warn" => Ok(InnerArgs::Warn),
            "scient" => Ok(InnerArgs::Scientific),
            "hex" => Ok(InnerArgs::Hex),
            "bin" => Ok(InnerArgs::Binary),
            "octal" => Ok(InnerArgs::Octal),
            v => Err(v),
        }
    }
}
