use std::fmt::Display;

// Would it be better to just have it as a singular enum, or a trait?

/// A trait meant to unify the way in which parts of the program are printed
pub trait Formattable {
    fn to_fmt(&self) -> Formatted;
}

//TEST: May change in form but a general print format is needed
pub enum Formatted {
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
    Integer,
    Float,
    Sized,
    Unsized,
    Char,
    Str,
    Bool,
    Nil,
    Any,
    BigInt,
    BigFloat,
    List,
    Map,
    Set,
    Tuple,
    Struct,
    Enum,
    Import,
    Export,
    Bind,
    Alias,
    Const,
    Var,
    Nest,
    Self_,
    Complex,
    Override,
    IsEmpty,
    IsWhitespace,
    FuncRange,
    FuncStartsW,
    FuncEndsW,
    FuncContains,
    FuncEquals,
    Cond,
    UserFunc,
    // Hmm...
    Nothing,
}

impl Display for Formatted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Formatted::Integer
            | Formatted::I8
            | Formatted::U8
            | Formatted::I16
            | Formatted::U16
            | Formatted::I32
            | Formatted::U32
            | Formatted::I64
            | Formatted::U64
            | Formatted::I128
            | Formatted::U128
            | Formatted::Sized
            | Formatted::BigInt
            | Formatted::Unsized => write!(f, "Integer"),
            Formatted::F16
            | Formatted::F32
            | Formatted::Float
            | Formatted::F64
            | Formatted::F128
            | Formatted::BigFloat => write!(f, "Float"),
            Formatted::Char => write!(f, "char"),
            Formatted::Str => write!(f, "str"),
            Formatted::Bool => write!(f, "bool"),
            Formatted::Nil => write!(f, "nil"),
            Formatted::Any => write!(f, "Any"),
            Formatted::List => write!(f, "List"),
            Formatted::Map => write!(f, "Map"),
            Formatted::Set => write!(f, "Set"),
            Formatted::Struct => write!(f, "struct"),
            Formatted::Enum => write!(f, "enum"),
            Formatted::Import => write!(f, "import"),
            Formatted::Export => write!(f, "export"),
            Formatted::Bind => write!(f, "bind"),
            Formatted::Alias => write!(f, "alias"),
            Formatted::Const => write!(f, "const"),
            Formatted::Var => write!(f, "variable"),
            Formatted::Nest => write!(f, "nest"),
            Formatted::Complex => write!(f, "complex"),
            Formatted::Override => write!(f, "Override"),
            Formatted::IsEmpty => write!(f, "IsEmpty"),
            Formatted::IsWhitespace => write!(f, "IsWhitespace"),
            Formatted::FuncRange => write!(f, "Range"),
            Formatted::FuncStartsW => write!(f, "StartsW"),
            Formatted::FuncEndsW => write!(f, "EndsW"),
            Formatted::FuncContains => write!(f, "Contains"),
            Formatted::FuncEquals => write!(f, "Equals"),
            Formatted::Cond => write!(f, "condition"),
            Formatted::UserFunc => write!(f, "function"),
            Formatted::Nothing => write!(f, ""),
            Formatted::Tuple => write!(f, "tuple"),
            Formatted::Self_ => write!(f, "self"),
        }
    }
}
