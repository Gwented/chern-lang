use std::marker::PhantomData;

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
