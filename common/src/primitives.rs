// Turn to macro or , or := := :=
pub enum PrimitiveKeywords {
    I8 = 0,
    U8 = 1,
    I16 = 2,
    U16 = 3,
    F16 = 4,
    I32 = 5,
    U32 = 6,
    F32 = 7,
    I64 = 8,
    U64 = 9,
    F64 = 10,
    I128 = 11,
    U128 = 12,
    F128 = 13,
    Sized = 14,
    Unsized = 15,
    Char = 16,
    Str = 17,
    Bool = 18,
    Nil = 19,
    BigInt = 20,
    BigFloat = 21,
    List = 22,
    Map = 23,
    Set = 24,
    //TODO:
    // struct = 25
    // enum = 26
    Bind = 25,
    Var = 26,
    Nest = 27,
    ComplexRules = 28,
    IsEmpty = 29,
    IsWhitespace = 30,
    Range = 31,
    StartsW = 32,
    EndsW = 33,
    Contains = 34,
}
//FIX: Maybe should reserve function names for checks

impl PrimitiveKeywords {
    pub fn from_id(id: u32) -> Option<PrimitiveKeywords> {
        match id {
            0 => Some(PrimitiveKeywords::I8),
            1 => Some(PrimitiveKeywords::U8),
            2 => Some(PrimitiveKeywords::I16),
            3 => Some(PrimitiveKeywords::U16),
            4 => Some(PrimitiveKeywords::F16),
            5 => Some(PrimitiveKeywords::I32),
            6 => Some(PrimitiveKeywords::U32),
            7 => Some(PrimitiveKeywords::F32),
            8 => Some(PrimitiveKeywords::I64),
            9 => Some(PrimitiveKeywords::U64),
            10 => Some(PrimitiveKeywords::F64),
            11 => Some(PrimitiveKeywords::I128),
            12 => Some(PrimitiveKeywords::U128),
            13 => Some(PrimitiveKeywords::F128),
            14 => Some(PrimitiveKeywords::Sized),
            15 => Some(PrimitiveKeywords::Unsized),
            16 => Some(PrimitiveKeywords::Char),
            17 => Some(PrimitiveKeywords::Str),
            18 => Some(PrimitiveKeywords::Bool),
            19 => Some(PrimitiveKeywords::Nil),
            20 => Some(PrimitiveKeywords::BigInt),
            21 => Some(PrimitiveKeywords::BigFloat),
            22 => Some(PrimitiveKeywords::List),
            23 => Some(PrimitiveKeywords::Map),
            24 => Some(PrimitiveKeywords::Set),
            //TODO:
            // struct = 25
            // enum = 26
            25 => Some(PrimitiveKeywords::Bind),
            26 => Some(PrimitiveKeywords::Var),
            27 => Some(PrimitiveKeywords::Nest),
            28 => Some(PrimitiveKeywords::ComplexRules),
            29 => Some(PrimitiveKeywords::IsEmpty),
            30 => Some(PrimitiveKeywords::IsWhitespace),
            31 => Some(PrimitiveKeywords::Range),
            32 => Some(PrimitiveKeywords::StartsW),
            33 => Some(PrimitiveKeywords::EndsW),
            34 => Some(PrimitiveKeywords::Contains),
            _ => None,
        }
    }
}
