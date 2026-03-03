use std::collections::HashMap;

use common::symbols::{SymbolId, TypeIdent};

use crate::token::{ActualPrimitives, Template};

//FIXME:
//MOVE ALL TO COMMON

#[derive(Debug)]
pub(crate) enum Symbol {
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
    // In case table has something else added
    pub(crate) fn new() -> SymbolTable {
        // I don't know. Anything but the enums.
        // FIX: This could not possibly end well
        SymbolTable {
            symbols: HashMap::new(),
            typedefs: Vec::new(),
            templates: Vec::new(),
            funcs: Vec::new(),
            primitives: Vec::new(),
        }
    }

    /// Direct reference to `SymbolTable` symbols
    pub(crate) fn symbols(&self) -> &HashMap<u32, Symbol> {
        &self.symbols
    }

    /// Direct reference to `SymbolTable` primitives
    pub(crate) fn type_ids(&self) -> &Vec<ActualPrimitives> {
        &self.primitives
    }
    //TODO: Maybe for all of the inner

    /// Stores `ActualPrimitives` and returns it's assigned type id
    pub(crate) fn store_primitive(&mut self, actual_type: ActualPrimitives) -> TypeIdent {
        let type_id = self.primitives.len();
        self.primitives.push(actual_type);

        TypeIdent::new(type_id as u32)
    }

    /// Stores `TypeDef` and returns it's assigned type id
    pub(crate) fn store_typedef(&mut self, type_def: TypeDef) -> TypeIdent {
        let type_id = self.typedefs.len();
        self.typedefs.push(type_def);
        TypeIdent::new(type_id as u32)
    }

    /// Stores `Template` and returns it's assigned type id
    pub(crate) fn store_template(&mut self, template: Template) -> TypeIdent {
        let type_id = self.templates.len();
        self.templates.push(template);

        TypeIdent::new(type_id as u32)
    }

    /// Stores `FuncDef` and returns it's assigned type id
    pub(crate) fn store_func(&mut self, func: FuncDef) -> TypeIdent {
        let sym_id = self.funcs.len();
        self.funcs.push(func);

        TypeIdent::new(sym_id as u32)
    }

    /// Stores `Symbol` which doesn't need a particular id since it's only looked up upon a valid
    /// given identifier
    pub(crate) fn store_symbol(&mut self, sym_id: SymbolId, symbol: Symbol) {
        self.symbols.insert(sym_id.id, symbol);
    }

    pub(crate) fn get_symbol(&self, sym_id: SymbolId) -> Option<&Symbol> {
        self.symbols.get(&sym_id.id)
    }

    // Remove?
    pub(crate) fn get_symbol_mut(&mut self, sym_id: SymbolId) -> Option<&mut Symbol> {
        self.symbols.get_mut(&sym_id.id)
    }

    /// ADD THE ERROR NOW
    /// No
    //FIX: Will return err. All temp.
    pub(crate) fn get_typedef_id(&self, sym_id: SymbolId) -> Option<TypeIdent> {
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
    pub(crate) fn get_template_id(&self, type_def_id: TypeIdent) -> Option<TypeIdent> {
        let type_def = self.extract_typedef(type_def_id);

        match self.templates.get(type_def.type_id.id as usize) {
            Some(_) => Some(type_def.type_id),
            None => None,
        }
    }

    pub(crate) fn extract_primitive(&self, type_id: TypeIdent) -> &ActualPrimitives {
        &self.primitives[type_id.id as usize]
    }

    pub(crate) fn extract_type_mut(&mut self, type_id: TypeIdent) -> &mut ActualPrimitives {
        &mut self.primitives[type_id.id as usize]
    }

    pub(crate) fn extract_typedef(&self, type_id: TypeIdent) -> &TypeDef {
        &self.typedefs[type_id.id as usize]
    }

    pub(crate) fn extract_typedef_mut(&mut self, type_id: TypeIdent) -> &mut TypeDef {
        &mut self.typedefs[type_id.id as usize]
    }

    pub(crate) fn extract_func(&self, type_id: TypeIdent) -> &FuncDef {
        &self.funcs[type_id.id as usize]
    }

    // Is this needed?
    pub(crate) fn extract_func_mut(&mut self, type_id: TypeIdent) -> &mut FuncDef {
        &mut self.funcs[type_id.id as usize]
    }

    pub(crate) fn extract_template(&self, type_id: TypeIdent) -> &Template {
        &self.templates[type_id.id as usize]
    }

    pub(crate) fn extract_template_mut(&mut self, type_id: TypeIdent) -> &mut Template {
        &mut self.templates[type_id.id as usize]
    }
}

#[derive(Debug)]
pub(crate) enum FuncArgs {
    Id(SymbolId),
    Literal(SymbolId),
    Num(usize),
}

#[derive(Debug)]
//FIX: Give interner a list of pathbufs
pub struct Bind {
    pub(crate) name_id: SymbolId,
}

impl Bind {
    pub(crate) fn new(id: SymbolId) -> Bind {
        Bind { name_id: id }
    }
}

#[derive(Debug)]
pub struct TypeDef {
    // May be integer idk
    pub(crate) sym_id: SymbolId,
    pub(crate) type_id: TypeIdent,
    pub(crate) args: Vec<InnerArgs>,
    pub(crate) cond: Vec<Cond>,
}

impl TypeDef {
    pub(crate) fn new(
        name_id: SymbolId,
        type_id: TypeIdent,
        args: Vec<InnerArgs>,
        cond: Vec<Cond>,
    ) -> TypeDef {
        TypeDef {
            sym_id: name_id,
            type_id,
            args,
            cond,
        }
    }
}

//FIX: Move
#[derive(Debug)]
pub(crate) enum Cond {
    Func(TypeIdent),
    // Maybe this shouldn't be a function
    IsEmpty,
    // Probably should just attach bool
    // should likely be removed
    Not(Box<Cond>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum InnerArgs {
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

#[derive(Debug)]
pub(crate) struct FuncDef {
    pub(crate) name_id: SymbolId,
    pub(crate) args: Vec<FuncArgs>,
}

impl FuncDef {
    pub(crate) fn new(name_id: SymbolId, args: Vec<FuncArgs>) -> FuncDef {
        FuncDef { name_id, args }
    }
}
