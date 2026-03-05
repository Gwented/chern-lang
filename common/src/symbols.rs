#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymbolId {
    pub id: u32,
}

impl SymbolId {
    pub fn new(id: u32) -> SymbolId {
        SymbolId { id }
    }
}

// Because of TypeId in Crust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeIdent {
    pub id: u32,
}

impl TypeIdent {
    pub fn new(id: u32) -> TypeIdent {
        TypeIdent { id }
    }
}

impl From<u32> for TypeIdent {
    fn from(v: u32) -> Self {
        TypeIdent::new(v)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScopeId {
    id: u32,
}
impl ScopeId {
    pub fn new(id: u32) -> ScopeId {
        ScopeId { id }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NameId {
    id: u32,
}

impl NameId {
    pub fn new(id: u32) -> NameId {
        NameId { id }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FuncId {
    id: u32,
}

impl FuncId {
    pub fn new(id: u32) -> FuncId {
        FuncId { id }
    }
}

//FIX: Should maybe be somewhere else but fine for now
#[derive(Debug, Clone)]
pub struct Span {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Span {
        Span { start, end }
    }
}

#[derive(Debug)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InnerArgs {
    Warn,
    Scientific,
    Hex,
    Binary,
    Octo,
}

impl<'a> TryFrom<&'a str> for InnerArgs {
    type Error = &'a str;

    fn try_from(v: &'a str) -> Result<Self, Self::Error> {
        match v {
            "warn" => Ok(InnerArgs::Warn),
            "scient" => Ok(InnerArgs::Scientific),
            "hex" => Ok(InnerArgs::Hex),
            "bin" => Ok(InnerArgs::Binary),
            "octo" => Ok(InnerArgs::Octo),
            v => Err(v),
        }
    }
}

// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub struct TemplateId {
//     pub id: u32,
// }
//
// impl TemplateId {
//     pub fn new(id: u32) -> TemplateId {
//         TemplateId { id }
//     }
// }
//
// #[derive(Debug, Clone, Copy, PartialEq, Eq)]
// pub struct TypeDefId {
//     pub id: u32,
// }
//
// impl TypeDefId {
//     pub fn new(id: u32) -> TypeDefId {
//         TypeDefId { id }
//     }
// }

// pub struct TypedTypeId<T> {
//     pub id: u32,
//     _phantom_data: PhantomData<T>,
// }
//
// impl<T> TypedTypeId<T> {
//     pub fn new(type_id: u32) -> TypedTypeId<T> {
//         TypedTypeId {
//             id: type_id,
//             _phantom_data: PhantomData,
//         }
//     }
// }
