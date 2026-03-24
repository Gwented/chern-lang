// TypeId is the index of the type itself, OR the type it's pointing to
use core::error;
use std::{collections::HashMap, fmt::Display};

use common::{
    builtins::{BuiltinType, BuiltinTypeKind},
    keywords,
    symbols::{
        AstId, BuiltinTypeId, EnumId, FuncId, InnerArgs, NameId, Span, SpannedInnerArgs, StructId,
        SymbolId, TypeDefId, TypeId,
    },
};

use crate::{semantic::constraints::ArgConstraint, types::symbols::Cond};

// What is a drop? I am new to thinking i have never thought before what is RAII
// is that a gui framework
// Maybe named, global table, program table

#[derive(Debug)]
pub(super) enum Type {
    BuiltinType(BuiltinType),
    Struct(SymbolId),
    Enum(SymbolId),
    Func(SymbolId),
    Alias(SymbolId),
    Tuple(Tuple),
    Unknown,
}

#[derive(Debug)]
pub(super) enum Symbol {
    TypeDef(TypeDefRepre),
    Struct(StructRepre),
    Func(FuncRepre),
    Enum(EnumRepre),
    Alias(AliasRepre),
}

#[derive(Debug)]
pub struct Table {
    pub(super) name_ids: HashMap<AstId, NameId>,
    // Can still change some to vec maybe
    pub(super) sym_ids: HashMap<AstId, SymbolId>,
    pub(super) symbols: HashMap<SymbolId, Symbol>,
    pub(super) types: Vec<Type>,
}

impl Table {
    pub fn new() -> Table {
        let mut table = Table {
            name_ids: HashMap::new(),
            sym_ids: HashMap::new(),
            symbols: HashMap::new(),
            types: Vec::new(),
        };

        // TEST: Taking away the data structures with - 3
        for i in 0..keywords::TYPE_END - 3 {
            let ty = BuiltinType::try_from_id(i as u32).expect("Builtin type not updated");
            table.types.push(Type::BuiltinType(ty));
        }

        table
    }

    // Is there a reason to return err?
    pub(super) fn get_typedef(&self, sym_id: SymbolId) -> &TypeDefRepre {
        match &self.symbols[&sym_id] {
            symbol => match symbol {
                Symbol::TypeDef(type_def_repre) => type_def_repre,
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_typedef_mut(&mut self, sym_id: SymbolId) -> &mut TypeDefRepre {
        match self.symbols.get_mut(&sym_id) {
            Some(symbol) => match symbol {
                Symbol::TypeDef(type_def_repre) => type_def_repre,
                _ => unreachable!(),
            },
            _ => unreachable!(),
        }
    }

    pub(super) fn get_struct(&self, sym_id: SymbolId) -> &StructRepre {
        match self.symbols.get(&sym_id) {
            Some(symbol) => match symbol {
                Symbol::Struct(struct_repre) => struct_repre,
                _ => unreachable!(),
            },
            None => unreachable!(),
        }
    }

    pub(super) fn get_struct_mut(&mut self, sym_id: SymbolId) -> &mut StructRepre {
        match self.symbols.get_mut(&sym_id) {
            Some(symbol) => match symbol {
                Symbol::Struct(struct_repre) => struct_repre,
                _ => unreachable!(),
            },
            None => unreachable!(),
        }
    }

    pub(super) fn get_func(&self, sym_id: SymbolId) -> &FuncRepre {
        match &self.symbols[&sym_id] {
            symbol => match symbol {
                Symbol::Func(func_repre) => func_repre,
                // Symbol::TypeDef(type_def_repre) => todo!(),
                // Symbol::Struct(struct_repre) => Some(struct_repre),
                // Symbol::Enum(enum_repre) => todo!(),
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_func_mut(&mut self, sym_id: SymbolId) -> &mut FuncRepre {
        match self.symbols.get_mut(&sym_id) {
            Some(symbol) => match symbol {
                Symbol::Func(func_repre) => func_repre,
                // Symbol::TypeDef(type_def_repre) => todo!(),
                // Symbol::Struct(struct_repre) => Some(struct_repre),
                // Symbol::Enum(enum_repre) => todo!(),
                _ => unreachable!(),
            },
            None => unreachable!(),
        }
    }

    pub(super) fn get_enum(&self, sym_id: SymbolId) -> &EnumRepre {
        match &self.symbols[&sym_id] {
            symbol => match symbol {
                Symbol::Enum(enum_repre) => enum_repre,
                // Symbol::Func(func_repre) => Some(func_repre),
                // Symbol::TypeDef(type_def_repre) => todo!(),
                // Symbol::Struct(struct_repre) => Some(struct_repre),
                _ => unreachable!(),
            },
        }
    }

    pub(super) fn get_enum_mut(&mut self, sym_id: SymbolId) -> &mut EnumRepre {
        match self.symbols.get_mut(&sym_id) {
            Some(symbol) => match symbol {
                Symbol::Enum(enum_repre) => enum_repre,
                // Symbol::Func(func_repre) => Some(func_repre),
                // Symbol::TypeDef(type_def_repre) => todo!(),
                // Symbol::Struct(struct_repre) => Some(struct_repre),
                _ => unreachable!(),
            },
            None => unreachable!(),
        }
    }
}

#[derive(Debug)]
pub(super) struct StructRepre {
    pub(super) name_id: NameId,
    pub(super) sym_id: SymbolId,
    pub(super) ast_id: AstId,
    // It's position in the Type array
    pub(super) type_id: TypeId,
    pub(super) fields: Vec<FieldRepre>,
    pub(super) args: Vec<InnerArgs>,
    pub(super) conds: Vec<Cond>,
}

impl StructRepre {
    pub fn new(
        name_id: NameId,
        sym_id: SymbolId,
        ast_id: AstId,
        type_id: TypeId,
        fields: Vec<FieldRepre>,
    ) -> StructRepre {
        StructRepre {
            name_id,
            sym_id,
            ast_id,
            type_id,
            fields,
            args: Vec::new(),
            conds: Vec::new(),
        }
    }

    // Likely too complex to be handled inside like this and should maybe be given a baked version
    // so that it can focus on checking arg types or the keyword of the cond.
    // pub fn supports_cond(&self, cond: Cond) -> bool {
    //     match cond {
    //         Cond::IsEmpty => todo!(),
    //         Cond::IsWhitespace => todo!(),
    //         Cond::Func(func_id) => todo!(),
    //         Cond::Not(cond) => todo!(),
    //     }
    // }
}

#[derive(Debug)]
pub(super) struct EnumRepre {
    pub(super) name_id: NameId,
    // Unsure about this positioning, I am hallucinating.
    pub(super) sym_id: SymbolId,
    pub(super) ast_id: AstId,
    pub(super) type_id: TypeId,
    pub(super) variants: Vec<VariantRepre>,
    pub(super) args: Vec<InnerArgs>,
    pub(super) conds: Vec<Cond>,
}

impl EnumRepre {
    pub fn new(
        name_id: NameId,
        sym_id: SymbolId,
        ast_id: AstId,
        type_id: TypeId,
        variants: Vec<VariantRepre>,
    ) -> EnumRepre {
        EnumRepre {
            name_id,
            sym_id,
            ast_id,
            type_id,
            variants,
            args: Vec::new(),
            conds: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct VariantRepre {
    pub(super) name_id: NameId,
    // Because enum types are nullable
    pub(super) type_id: Option<TypeId>,
    // Possible tuple
    // Points to variant within original Ast enum
    pub(super) ast_id: AstId,
    pub(super) args: Vec<InnerArgs>,
    pub(super) conds: Vec<Cond>,
}

impl VariantRepre {
    pub fn new(name_id: NameId, type_id: Option<TypeId>, ast_id: AstId) -> VariantRepre {
        VariantRepre {
            name_id,
            type_id,
            ast_id,
            args: Vec::new(),
            conds: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub(super) struct TypeDefRepre {
    pub(super) name_id: NameId,
    pub(super) sym_id: SymbolId,
    pub(super) ast_id: AstId,
    pub(super) type_id: TypeId,
    pub(super) conds: Vec<Cond>,
    pub(super) args: Vec<InnerArgs>,
}

impl TypeDefRepre {
    pub fn new(name_id: NameId, type_id: TypeId, sym_id: SymbolId, ast_id: AstId) -> TypeDefRepre {
        TypeDefRepre {
            name_id,
            sym_id,
            ast_id,
            type_id,
            conds: Vec::new(),
            args: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct FuncRepre {
    pub(super) name_id: NameId,
    // Type reference to it's own position in the `Type` array
    pub(super) type_id: TypeId,
    pub(super) call_span: Span,
    pub(super) kind: FuncKind,
    pub(super) constraints: Vec<ArgConstraint>,
    pub(super) args: Vec<FuncArgsRepre>,
}

impl FuncRepre {
    pub(super) fn new(
        name_id: NameId,
        type_id: TypeId,
        call_span: Span,
        kind: FuncKind,
        constraints: Vec<ArgConstraint>,
        args: Vec<FuncArgsRepre>,
    ) -> FuncRepre {
        FuncRepre {
            name_id,
            type_id,
            kind,
            call_span,
            constraints,
            args,
        }
    }
}

// I'm scared of this
#[derive(Debug)]
pub(super) enum FuncArgsRepre {
    Integer(i64),
    Float(f64),
    Char(char),
    //TEST:
    Var(TypeId, BuiltinTypeKind),
    Str(NameId),
}

// KIND VARIANT IS NOT, NEEDED. PLEASE.
impl FuncArgsRepre {
    pub(super) fn is_numeric(&self) -> bool {
        match self {
            FuncArgsRepre::Integer(_) | FuncArgsRepre::Float(_) => true,
            FuncArgsRepre::Var(_, kind) => kind.is_numeric(),

            FuncArgsRepre::Char(_) | FuncArgsRepre::Str(_) => false,
        }
    }

    // TEST: Currently testing out a way of formatting that is more encapsulated so that the code
    // outside doesn't have to be repeated as intensely.
    pub(super) fn is_integer(&self) -> bool {
        match self.kind() {
            FuncArgsKind::Integer => true,
            _ => false,
        }
    }

    pub(super) fn is_float(&self) -> bool {
        match self.kind() {
            FuncArgsKind::Float => true,
            _ => false,
        }
    }

    pub(super) fn is_char(&self) -> bool {
        match self.kind() {
            FuncArgsKind::Char => true,
            _ => false,
        }
    }

    pub(super) fn is_str(&self) -> bool {
        match self.kind() {
            FuncArgsKind::Str => true,
            _ => false,
        }
    }

    // to_builtin_type_kind is getting a little long for something so contextually obvious
    pub(super) fn to_builtin_kind(&self) -> BuiltinTypeKind {
        match self {
            FuncArgsRepre::Integer(_) => BuiltinTypeKind::I64,
            FuncArgsRepre::Float(_) => BuiltinTypeKind::F64,
            FuncArgsRepre::Char(_) => BuiltinTypeKind::Char,
            FuncArgsRepre::Var(_, kind) => *kind,
            FuncArgsRepre::Str(_) => BuiltinTypeKind::Str,
        }
    }

    pub(super) fn kind(&self) -> FuncArgsKind {
        match self {
            FuncArgsRepre::Integer(_) => FuncArgsKind::Integer,
            FuncArgsRepre::Float(_) => FuncArgsKind::Float,
            FuncArgsRepre::Char(_) => FuncArgsKind::Char,
            FuncArgsRepre::Var(_, _) => FuncArgsKind::Var,
            FuncArgsRepre::Str(_) => FuncArgsKind::Str,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FuncArgsKind {
    Integer,
    Float,
    Char,
    Var,
    Str,
}

#[derive(Debug)]
pub(super) struct FieldRepre {
    pub(super) name_id: NameId,
    pub(super) type_id: TypeId,
    // Ast contained field id, maybe this should just be AstId
    pub(super) ast_id: AstId,
}

impl FieldRepre {
    pub fn new(name_id: NameId, ty: TypeId, ast_id: AstId) -> FieldRepre {
        FieldRepre {
            name_id,
            type_id: ty,
            ast_id,
        }
    }
}

#[derive(Debug)]
pub(super) struct AliasRepre {
    pub(crate) name_id: NameId,
    pub(super) sym_id: SymbolId,
    pub(super) ast_id: AstId,
    // Type id is a little wrong
    pub(super) type_id: TypeId,
    pub(crate) params: Vec<TypeId>,
    pub(super) conds: Vec<Cond>,
    pub(super) args: Vec<InnerArgs>,
}

// impl AliasRepre {
//     pub fn new() -> AliasRepre {}
// }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FuncKind {
    Contains,
    Range,
    StartsW,
    EndsW, // UserDefined
    UserDefined,
}

impl Display for FuncKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FuncKind::Contains => write!(f, "Contains"),
            FuncKind::Range => write!(f, "Range"),
            FuncKind::StartsW => write!(f, "StartsW"),
            FuncKind::EndsW => write!(f, "EndsW"),
            FuncKind::UserDefined => write!(f, "<Hi>"),
        }
    }
}

#[derive(Debug)]
pub(super) struct Tuple {
    pub(super) elements: Vec<TypeId>,
    pub(super) type_id: TypeId,
}

impl Tuple {
    pub fn new(elements: Vec<TypeId>, type_id: TypeId) -> Tuple {
        Tuple { elements, type_id }
    }
}
