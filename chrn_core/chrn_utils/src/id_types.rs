#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InternedId {
    pub id: u32,
}

impl InternedId {
    pub fn new(id: u32) -> InternedId {
        InternedId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PathId {
    pub id: u32,
}

impl PathId {
    pub fn new(id: u32) -> PathId {
        PathId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct TypeId {
    pub id: u32,
}

impl TypeId {
    pub fn new(id: u32) -> TypeId {
        TypeId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolId {
    pub id: u32,
}

impl SymbolId {
    pub fn new(id: u32) -> SymbolId {
        SymbolId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ModuleId {
    pub id: usize,
}

impl ModuleId {
    pub fn new(id: usize) -> ModuleId {
        ModuleId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ScopeId {
    pub id: usize,
}

impl ScopeId {
    pub fn new(id: usize) -> ScopeId {
        ScopeId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AstId {
    pub id: u32,
}

impl AstId {
    pub fn new(id: u32) -> AstId {
        AstId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FieldId {
    pub id: u32,
}

impl FieldId {
    pub fn new(id: u32) -> FieldId {
        FieldId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ExprId {
    pub id: u32,
}

impl ExprId {
    pub fn new(id: u32) -> ExprId {
        ExprId { id }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ValueId {
    pub id: u32,
}

impl ValueId {
    pub fn new(id: u32) -> ValueId {
        ValueId { id }
    }
}
