use std::fmt::Display;

use common::span::Span;

use crate::builtins::BuiltinType;

// This isn't an accurate name anymore...
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeId {
    pub id: u32,
}

impl TypeId {
    pub fn new(id: u32) -> TypeId {
        TypeId { id }
    }
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
pub struct ModuleId {
    pub id: usize,
}

impl ModuleId {
    pub fn new(id: usize) -> ModuleId {
        ModuleId { id }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ValueId {
    pub id: usize,
}

impl ValueId {
    pub fn new(id: usize) -> ValueId {
        ValueId { id }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PathId {
    pub id: u32,
}

impl PathId {
    pub fn new(id: u32) -> PathId {
        PathId { id }
    }
}

// pub struct SpannedNameId {
//     pub name_id: NameId,
//     pub span: Span,
// }
//
// impl SpannedNameId {
//     pub fn new(name_id: NameId, span: Span) -> SpannedNameId {
//         SpannedNameId { name_id, span }
//     }
// }

// pub struct SpannedBuiltinType {
//     pub ty: BuiltinType,
//     pub span: Span,
// }
//
// impl SpannedBuiltinType {
//     pub fn new(ty: BuiltinType, span: Span) -> SpannedBuiltinType {
//         SpannedBuiltinType { ty, span }
//     }
// }
//
