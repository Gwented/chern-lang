use crate::keywords::Keyword;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

pub static ARGS_ARRAY: [&str; 5] = ["warn", "scient", "hex", "bin", "octal"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InnerArgs {
    Warn,
    Scientific,
    Hex,
    Binary,
    Octal,
}

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
