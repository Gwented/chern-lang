use std::collections::HashMap;

use common::{
    builtins::{BuiltinType, BuiltinTypeKind},
    fmter::{Formattable, Formatted},
    keywords::Keyword,
    symbols::{
        BuiltinTypeId, EnumId, FuncId, InnerArgs, NameId, Span, StructId, SymbolId, TypeDefId,
        TypeId,
    },
};

use crate::{
    semantic::representation::{FuncArgsKind, FuncArgsRepre, FuncKind},
    types::token::Token,
};

//WARN: THERE ARE MANY WAYS OF DOING THIS SO I AM JUST CHOOSING THIS FOR NOW I AM VERY CONFUSED
//MAY REMOVE
// #[derive(Debug, Clone)]
// pub enum Symbol {
//     TypeDef(UnresolvedId),
//     Struct(UnresolvedId),
//     Enum(UnresolvedId),
//     Func(UnresolvedId),
//     Prim(UnresolvedId),
// }

#[derive(Debug)]
pub struct SymbolTable {
    //Can just be a vec?
    // pub(super) sym_table: HashMap<NameId, Symbol>,
    pub(super) typedefs: Vec<TypeDef>,
    pub(super) structs: Vec<Structure>,
    pub(super) funcs: Vec<FuncDef>,
    //WARN: May merge structs and enums
    pub(super) enums: Vec<Enum>,
    // I know this has more than primitives.
    pub(super) primitives: Vec<BuiltinType>,
}

#[derive(Debug, Clone)]
pub struct SpannedToken {
    pub(crate) tok: Token,
    pub span: Span,
}

// Ok
// TODO: MOVE THIS TO SEMANTIC MAYBE, I THINK
impl SymbolTable {
    pub fn new() -> SymbolTable {
        //TODO: Is the same needed or symbols?
        let mut sym_table = SymbolTable {
            typedefs: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            // Push known funks?
            funcs: Vec::new(),
            primitives: Vec::new(),
            // sym_table: todo!(),
        };

        sym_table
    }

    // pub fn symbols(&self) -> &HashMap<u32, Symbol> {
    //     &self.symbols
    // }
    //
    // /// Direct reference to `SymbolTable` primitives
    // pub fn type_ids(&self) -> &Vec<ActualPrimitives> {
    //     &self.primitives
    // }
    // //TODO: Maybe for all of the inner
    //
    // /// Stores `ActualPrimitives` and returns it's assigned type id
    // pub fn store_primitive(&mut self, actual_type: ActualPrimitives) -> TypeIdent {
    //     let type_id = self.primitives.len();
    //     self.primitives.push(actual_type);
    //
    //     TypeIdent::new(type_id as u32)
    // }
    //
    // /// Stores `TypeDef` and returns it's assigned type id
    // pub fn store_typedef(&mut self, type_def: TypeDef) -> TypeIdent {
    //     let type_id = self.typedefs.len();
    //     self.typedefs.push(type_def);
    //     TypeIdent::new(type_id as u32)
    // }
    //
    // /// Stores `Template` and returns it's assigned type id
    // pub fn store_template(&mut self, structure: Structure) -> TypeIdent {
    //     let type_id = self.structs.len();
    //     self.structs.push(structure);
    //
    //     TypeIdent::new(type_id as u32)
    // }
    //
    // /// Stores `FuncDef` and returns it's assigned type id
    // pub fn store_func(&mut self, func: FuncDef) -> TypeIdent {
    //     let sym_id = self.funcs.len();
    //     self.funcs.push(func);
    //
    //     TypeIdent::new(sym_id as u32)
    // }
    //
    // /// Stores `Symbol` which doesn't need a particular id since it's only looked up upon a valid
    // /// given identifier
    // pub fn store_symbol(&mut self, sym_id: SymbolId, symbol: Symbol) {
    //     self.symbols.insert(sym_id.id, symbol);
    // }
    //
    // pub fn get_symbol(&self, sym_id: SymbolId) -> Option<&Symbol> {
    //     self.symbols.get(&sym_id.id)
    // }
    //
    // // Remove?
    // pub fn get_symbol_mut(&mut self, sym_id: SymbolId) -> Option<&mut Symbol> {
    //     self.symbols.get_mut(&sym_id.id)
    // }
    //
    // /// ADD THE ERROR NOW
    // /// No
    // //FIX: Will return err. All temp.
    //
    // //FIX: Will return err
    // /// Takes in a `TypeDef` id and return option template type id
    // /// TYPE ENFORCE THESE PLEASE
    //
    // pub fn extract_primitive(&self, type_id: TypeIdent) -> &ActualPrimitives {
    //     &self.primitives[type_id.id as usize]
    // }
    //
    // pub fn extract_primitive_mut(&mut self, type_id: TypeIdent) -> &mut ActualPrimitives {
    //     &mut self.primitives[type_id.id as usize]
    // }
    //
    // pub fn extract_typedef(&self, type_id: TypeIdent) -> &TypeDef {
    //     &self.typedefs[type_id.id as usize]
    // }
    //
    // pub fn extract_typedef_mut(&mut self, type_id: TypeIdent) -> &mut TypeDef {
    //     &mut self.typedefs[type_id.id as usize]
    // }

    // pub fn extract_func(&self, type_id: TypeIdent) -> &FuncDef {
    //     &self.funcs[type_id.id as usize]
    // }
    //
    // // Is this needed?
    // pub fn extract_func_mut(&mut self, type_id: TypeIdent) -> &mut FuncDef {
    //     &mut self.funcs[type_id.id as usize]
    // }
    //
    // pub fn extract_struct(&self, type_id: TypeIdent) -> &Structure {
    //     &self.structs[type_id.id as usize]
    // }
    //
    // pub fn extract_struct_mut(&mut self, type_id: TypeIdent) -> &mut Structure {
    //     &mut self.structs[type_id.id as usize]
    // }
    //
    // pub fn extract_enum(&self, enum_id: EnumId) -> &Enum {
    //     &self.enums[enum_id.id as usize]
    // }
    //
    // pub fn extract_enum_mut(&mut self, enum_id: EnumId) -> &mut Enum {
    //     &mut self.enums[enum_id.id as usize]
    // }
}

// #[derive(Debug)]
// //FIX: Give interner a list of pathbufs
// pub struct Bind {
//     pub name_id: NameId,
// }
//
// impl Bind {
//     pub fn new(name_id: NameId) -> Bind {
//         Bind { name_id }
//     }
// }
// To my understanding this is GETTING a symbol id not as the literal string name attachment,
// but as uh

//TODO:
// Unsure if serial needs this because the IR could be so simple that this isn't really necessary
#[derive(Debug)]
pub struct TypeDef {
    pub name_id: NameId,
    pub type_id: TypeId,
    pub args: Vec<InnerArgs>,
    pub conds: Vec<Cond>,
}

impl TypeDef {
    pub fn new(
        name_id: NameId,
        type_id: TypeId,
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
pub struct Structure {
    pub(crate) name_id: NameId,
    // pub(crate) type_id: TypeIdent,
    pub(crate) args: Vec<InnerArgs>,
    pub(crate) conds: Vec<Cond>,
    // Fields can be variants or separate strugg <-- Sgwom
    //WARN:
    // pub(crate) fields: Vec<TypeId>,
}

// impl Structure {
//     pub(crate) fn new(name_id: NameId, type_id: TypeIdent) -> Structure {
//         Structure {
//             name_id,
//             type_id,
//             args: Vec::new(),
//             conds: Vec::new(),
//             fields: Vec::new(),
//         }
//     }
// }
//
//TODO: Maybe name these resolved for clarity?
// Should parents have conditions?
#[derive(Debug)]
pub struct Enum {
    pub(crate) name_id: NameId,
    pub(crate) ty: SymbolId,
    pub(crate) args: Vec<InnerArgs>,
    pub(crate) conds: Vec<Cond>,
    // Fields can be variants or separate strugg <-- Sgwom
    pub(crate) variants: Vec<EnumId>,
}

//FIX:
#[derive(Debug)]
pub struct Variant {
    pub(crate) name_id: NameId,
    // I think this is right?
    pub(crate) ty: Option<SymbolId>,
    pub(crate) args: Vec<InnerArgs>,
    pub(crate) conds: Vec<Cond>,
}

impl Variant {
    pub fn new(
        name_id: NameId,
        // I think this is right?
        ty: Option<SymbolId>,
        args: Vec<InnerArgs>,
        conds: Vec<Cond>,
    ) -> Variant {
        Variant {
            name_id,
            ty,
            args,
            conds,
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

#[derive(Debug)]
pub enum FuncArgs {
    Var(SymbolId),
    Literal(SymbolId),
    Num(usize),
}

// Spanned cond can't exist because conds are expressions
#[derive(Debug, Clone)]
pub enum Cond {
    //FIX:
    Func(SymbolId, FuncKind),
    IsEmpty,
    IsWhitespace,
    Not(Box<Cond>),
}

impl Formattable for Cond {
    fn to_fmt(&self) -> common::fmter::Formatted {
        match self {
            Cond::IsEmpty => Formatted::IsEmpty,
            Cond::IsWhitespace => Formatted::IsWhitespace,
            Cond::Not(cond) => Formatted::Nothing,
            // Maybe pair with kind?
            Cond::Func(_, kind) => kind.to_fmt(),
        }
    }
}

// I'm actually fine with this.
impl Cond {
    /// Only returns a condition if it is solely a keyword, excluding any functional
    /// conditions.
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

    pub fn supports_builtin_type(&self, kind: BuiltinTypeKind) -> bool {
        match self {
            Cond::IsEmpty | Cond::IsWhitespace => {
                if kind.is_numeric() {
                    return false;
                }

                true
            }
            Cond::Not(cond) => Self::supports_builtin_type(cond, kind),
            Cond::Func(_, _) => unreachable!("Not a possible variant"),
        }
    }
}
