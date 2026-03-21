use std::fmt::Display;

// Would it be better to just have it as a singular enum, or a trait?

/// A trait meant to unify the way in which parts of the program are printed
pub trait Formatable {
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
    Struct,
    Enum,
    Import,
    Export,
    Bind,
    Alias,
    Const,
    Var,
    Nest,
    Complex,
    Override,
    IsEmpty,
    IsWhitespace,
    Range,
    StartsW,
    EndsW,
    Contains,
    // Hmm...
    Nothing,
}

impl Display for Formatted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Formatted::I8
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
            | Formatted::Unsized => write!(f, "integer"),
            Formatted::F16
            | Formatted::F32
            | Formatted::F64
            | Formatted::F128
            | Formatted::BigFloat => write!(f, "float"),
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
            Formatted::IsWhitespace => write!(f, "IsEmpty"),
            Formatted::Range => write!(f, "Range"),
            Formatted::StartsW => write!(f, "StartsW"),
            Formatted::EndsW => write!(f, "EndsW"),
            Formatted::Contains => write!(f, "Contains"),
            Formatted::Nothing => write!(f, ""),
        }
    }
}
