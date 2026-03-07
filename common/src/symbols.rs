use crate::token::{ActualPrimitives, Token};
use std::collections::HashMap;

//FIXME:
//MOVE ALL BACK TO SCRIPT

#[derive(Debug)]
pub enum Symbol {
    Bind(Bind),
    Func(TypeIdent),
    Def(TypeIdent),
}

// Dog dog = new Dog();
// TODO: Reserve index 0 for all to represent invalid types from the parser
#[derive(Debug)]
pub struct SymbolTable {
    //Can just be a vec?
    symbols: HashMap<u32, Symbol>,
    typedefs: Vec<TypeDef>,
    templates: Vec<Template>,
    funcs: Vec<FuncDef>,
    // I know this has more than primitives.
    primitives: Vec<ActualPrimitives>,
}

// trait Typed {}
//
// impl Typed for Bind {}
// impl Typed for Template {}
// impl Typed for FuncDef {}
// impl Typed for TypeDef {}
// impl Typed for ActualPrimitives {}

//TODO: Maybe traits for generics instead if possible
//No
impl SymbolTable {
    pub fn new() -> SymbolTable {
        //TODO: Is the same needed or symbols?
        let mut sym_table = SymbolTable {
            symbols: HashMap::new(),
            typedefs: Vec::new(),
            templates: Vec::new(),
            // Push known funs? I. Don't. Know.
            funcs: Vec::new(),
            primitives: Vec::new(),
        };

        sym_table
    }

    /// Direct reference to `SymbolTable` symbols
    pub fn symbols(&self) -> &HashMap<u32, Symbol> {
        &self.symbols
    }

    /// Direct reference to `SymbolTable` primitives
    pub fn type_ids(&self) -> &Vec<ActualPrimitives> {
        &self.primitives
    }
    //TODO: Maybe for all of the inner

    /// Stores `ActualPrimitives` and returns it's assigned type id
    pub fn store_primitive(&mut self, actual_type: ActualPrimitives) -> TypeIdent {
        let type_id = self.primitives.len();
        self.primitives.push(actual_type);

        TypeIdent::new(type_id as u32)
    }

    /// Stores `TypeDef` and returns it's assigned type id
    pub fn store_typedef(&mut self, type_def: TypeDef) -> TypeIdent {
        let type_id = self.typedefs.len();
        self.typedefs.push(type_def);
        TypeIdent::new(type_id as u32)
    }

    /// Stores `Template` and returns it's assigned type id
    pub fn store_template(&mut self, template: Template) -> TypeIdent {
        let type_id = self.templates.len();
        self.templates.push(template);

        TypeIdent::new(type_id as u32)
    }

    /// Stores `FuncDef` and returns it's assigned type id
    pub fn store_func(&mut self, func: FuncDef) -> TypeIdent {
        let sym_id = self.funcs.len();
        self.funcs.push(func);

        TypeIdent::new(sym_id as u32)
    }

    /// Stores `Symbol` which doesn't need a particular id since it's only looked up upon a valid
    /// given identifier
    pub fn store_symbol(&mut self, sym_id: SymbolId, symbol: Symbol) {
        self.symbols.insert(sym_id.id, symbol);
    }

    pub fn get_symbol(&self, sym_id: SymbolId) -> Option<&Symbol> {
        self.symbols.get(&sym_id.id)
    }

    // Remove?
    pub fn get_symbol_mut(&mut self, sym_id: SymbolId) -> Option<&mut Symbol> {
        self.symbols.get_mut(&sym_id.id)
    }

    /// ADD THE ERROR NOW
    /// No
    //FIX: Will return err. All temp.
    pub fn get_typedef_id(&self, sym_id: SymbolId) -> Option<TypeIdent> {
        let symbol = self.get_symbol(sym_id);

        match symbol {
            Some(sym) => match sym {
                Symbol::Def(type_ident) => Some(*type_ident),
                _ => None,
            },
            None => None,
        }
    }

    //FIX: Will return err
    /// Takes in a `TypeDef` id and return option template type id
    /// TYPE ENFORCE THESE PLEASE
    pub fn get_template_id(&self, type_def_id: TypeIdent) -> Option<TypeIdent> {
        let type_def = self.extract_typedef(type_def_id);

        match self.templates.get(type_def.type_id.id as usize) {
            Some(_) => Some(type_def.type_id),
            None => None,
        }
    }

    pub fn extract_primitive(&self, type_id: TypeIdent) -> &ActualPrimitives {
        &self.primitives[type_id.id as usize]
    }

    pub fn extract_primitive_mut(&mut self, type_id: TypeIdent) -> &mut ActualPrimitives {
        &mut self.primitives[type_id.id as usize]
    }

    pub fn extract_typedef(&self, type_id: TypeIdent) -> &TypeDef {
        &self.typedefs[type_id.id as usize]
    }

    pub fn extract_typedef_mut(&mut self, type_id: TypeIdent) -> &mut TypeDef {
        &mut self.typedefs[type_id.id as usize]
    }

    pub fn extract_func(&self, type_id: TypeIdent) -> &FuncDef {
        &self.funcs[type_id.id as usize]
    }

    // Is this needed?
    pub fn extract_func_mut(&mut self, type_id: TypeIdent) -> &mut FuncDef {
        &mut self.funcs[type_id.id as usize]
    }

    pub fn extract_template(&self, type_id: TypeIdent) -> &Template {
        &self.templates[type_id.id as usize]
    }

    pub fn extract_template_mut(&mut self, type_id: TypeIdent) -> &mut Template {
        &mut self.templates[type_id.id as usize]
    }
}

#[derive(Debug)]
pub enum FuncArgs {
    Id(SymbolId),
    Literal(SymbolId),
    Num(usize),
}

#[derive(Debug)]
//FIX: Give interner a list of pathbufs
pub struct Bind {
    pub name_id: NameId,
}

impl Bind {
    pub fn new(name_id: NameId) -> Bind {
        Bind { name_id }
    }
}
// To my understanding this is GETTING a symbol id not as the literal string name attachment,
// but as uh

//TODO:
//
#[derive(Debug)]
pub struct TypeDef {
    pub name_id: NameId,
    pub type_id: TypeIdent,
    pub args: Vec<InnerArgs>,
    pub conds: Vec<Cond>,
}

impl TypeDef {
    pub fn new(
        name_id: NameId,
        type_id: TypeIdent,
        args: Vec<InnerArgs>,
        conds: Vec<Cond>,
    ) -> TypeDef {
        TypeDef {
            name_id,
            type_id,
            args,
            conds,
        }
    }
}

#[derive(Debug)]
pub struct Template {
    // Should this be a symbol or type id?
    pub(crate) name_id: NameId,
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

#[derive(Debug, Clone, Copy)]
pub(crate) enum Repre {
    Struct,
    Enum,
}

impl Template {
    pub(crate) fn new(name_id: NameId, type_id: TypeIdent, repre: Repre) -> Template {
        Template {
            name_id,
            type_id,
            args: Vec::new(),
            conds: Vec::new(),
            fields: Vec::new(),
            repre,
        }
    }
}

#[derive(Debug)]
pub struct FuncDef {
    pub name_id: NameId,
    pub func_id: FuncId,
    pub args: Vec<FuncArgs>,
}

impl FuncDef {
    pub fn new(name_id: NameId, func_id: FuncId, args: Vec<FuncArgs>) -> FuncDef {
        FuncDef {
            name_id,
            func_id,
            args,
        }
    }
}

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
    pub id: u32,
}
impl ScopeId {
    pub fn new(id: u32) -> ScopeId {
        ScopeId { id }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

#[derive(Debug, Clone)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Span,
}

//FIX: Should maybe be somewhere else but fine for now
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
