use core::error;
use std::{collections::HashMap, fmt::Display};

use common::{
    builtins::BuiltinType,
    keywords,
    symbols::{
        AstId, BuiltinTypeId, Cond, EnumId, FuncId, InnerArgs, NameId, Span, SpannedInnerArgs,
        StructId, SymbolId, TypeDefId, TypedId,
    },
};

// What is a drop? I am new to thinking i have never thought before what is RAII
// is that a gui framework
// Maybe named, global table, program table
#[derive(Debug)]
pub struct Table {
    //FIXME:
    // Can likely change to arrays later but not priority
    pub(super) name_ids: HashMap<AstId, NameId>,
    pub(super) sym_ids: HashMap<AstId, SymbolId>,
    // This could just be a vec
    pub(super) typed_ids: HashMap<SymbolId, TypedId>,
    pub(super) typedefs: Vec<TypeDefRepre>,
    pub(super) structs: Vec<StructRepre>,
    pub(super) funcs: Vec<FuncRepre>,
    pub(super) enums: Vec<EnumRepre>,
    pub(super) builtin_types: Vec<BuiltinType>,
}

// This will be removed. Likely replaced by. Um. I don't know.
impl Table {
    pub fn new() -> Table {
        let mut table = Table {
            name_ids: HashMap::new(),
            sym_ids: HashMap::new(),
            typed_ids: HashMap::new(),
            typedefs: Vec::new(),
            structs: Vec::new(),
            funcs: Vec::new(),
            enums: Vec::new(),
            builtin_types: Vec::new(),
        };

        // TEST: Taking away the data structures with - 3
        for i in 0..keywords::TYPE_END - 3 {
            table
                .builtin_types
                .push(BuiltinType::try_from_id(i as u32).expect("Builtin type not updated"));
        }

        table
    }
}

#[derive(Debug)]
pub(super) struct StructRepre {
    pub(super) name_id: NameId,
    pub(super) sym_id: SymbolId,
    pub(super) ast_id: AstId,
    pub(super) fields: Vec<FieldRepre>,
    pub(super) args: Vec<InnerArgs>,
    pub(super) conds: Vec<Cond>,
}

impl StructRepre {
    pub fn new(
        name_id: NameId,
        sym_id: SymbolId,
        ast_id: AstId,
        fields: Vec<FieldRepre>,
    ) -> StructRepre {
        StructRepre {
            name_id,
            sym_id,
            ast_id,
            fields,
            args: Vec::new(),
            conds: Vec::new(),
        }
    }

    pub fn supports_arg(&self, arg: InnerArgs) -> bool {
        match arg {
            InnerArgs::Warn
            | InnerArgs::Scientific
            | InnerArgs::Hex
            | InnerArgs::Binary
            | InnerArgs::Octal => true,
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
    pub(super) variants: Vec<VariantRepre>,
    pub(super) args: Vec<InnerArgs>,
    pub(super) conds: Vec<Cond>,
}

impl EnumRepre {
    pub fn new(
        name_id: NameId,
        sym_id: SymbolId,
        ast_id: AstId,
        variants: Vec<VariantRepre>,
    ) -> EnumRepre {
        EnumRepre {
            name_id,
            sym_id,
            ast_id,
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
    pub(super) typed_id: Option<TypedId>,
    // Points to variant within original Ast enum
    pub(super) ast_id: AstId,
    pub(super) args: Vec<InnerArgs>,
    pub(super) conds: Vec<Cond>,
}

impl VariantRepre {
    pub fn new(name_id: NameId, typed_id: Option<TypedId>, ast_id: AstId) -> VariantRepre {
        VariantRepre {
            name_id,
            typed_id,
            ast_id,
            args: Vec::new(),
            conds: Vec::new(),
        }
    }

    pub fn supports_arg(&self, arg: InnerArgs) -> bool {
        match arg {
            InnerArgs::Warn
            | InnerArgs::Scientific
            | InnerArgs::Hex
            | InnerArgs::Binary
            | InnerArgs::Octal => true,
        }
    }
}

#[derive(Debug)]
pub(super) struct TypeDefRepre {
    pub(super) name_id: NameId,
    pub(super) sym_id: SymbolId,
    pub(super) ast_id: AstId,
    pub(super) typed_id: TypedId,
    pub(super) conds: Vec<Cond>,
    pub(super) args: Vec<InnerArgs>,
}

impl TypeDefRepre {
    pub fn new(
        name_id: NameId,
        typed_id: TypedId,
        sym_id: SymbolId,
        ast_id: AstId,
    ) -> TypeDefRepre {
        TypeDefRepre {
            name_id,
            sym_id,
            ast_id,
            typed_id,
            conds: Vec::new(),
            args: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub(super) struct FuncRepre {
    pub(super) name_id: NameId,
    pub(super) call_span: Span,
    pub(super) kind: FuncKind,
    pub(super) constraints: Vec<ArgConstraint>,
    pub(super) args: Vec<FuncArgsRepre>,
}

impl FuncRepre {
    pub(super) fn new(
        name_id: NameId,
        call_span: Span,
        kind: FuncKind,
        constraints: Vec<ArgConstraint>,
        args: Vec<FuncArgsRepre>,
    ) -> FuncRepre {
        FuncRepre {
            name_id,
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
    Var(SymbolId),
    Str(NameId),
}

// Is this right?
impl PartialEq for FuncArgsRepre {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Integer(_), Self::Integer(_))
            | (Self::Float(_), Self::Float(_))
            | (Self::Char(_), Self::Char(_))
            | (Self::Var(_), Self::Var(_))
            | (Self::Str(_), Self::Str(_)) => true,
            _ => false,
        }
    }
}

#[derive(Debug)]
pub(super) struct FieldRepre {
    pub(super) name_id: NameId,
    pub(super) ty: TypedId,
    // Ast contained field id, maybe this should just be AstId
    pub(super) ast_id: AstId,
}

impl FieldRepre {
    pub fn new(name_id: NameId, ty: TypedId, ast_id: AstId) -> FieldRepre {
        FieldRepre {
            name_id,
            ty,
            ast_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FuncKind {
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

// Nat
// Real
// Complex
// Prime
// TEST:
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ArgConstraint {
    ParamCount(u8),
    DynType,
    MatchingType,
    Numeric,
    Integer,
    Float,
    Str,
}

impl ArgConstraint {
    // PLEASE DONT MAKE ME RETURN OPTION
    // TEST:
    /// Takes in a function kind that is built in and returns it's constraints
    pub fn from_builtin(kind: FuncKind) -> Vec<ArgConstraint> {
        match kind {
            FuncKind::StartsW => {
                // Maybe if we got something like 0x1FF it could StartsW(0x1FF)?
                vec![ArgConstraint::ParamCount(1), ArgConstraint::MatchingType]
            }
            FuncKind::EndsW => {
                vec![ArgConstraint::ParamCount(1), ArgConstraint::MatchingType]
            }
            FuncKind::Contains => {
                vec![ArgConstraint::ParamCount(1), ArgConstraint::MatchingType]
            }
            FuncKind::Range => {
                vec![
                    ArgConstraint::ParamCount(2),
                    ArgConstraint::Numeric,
                    ArgConstraint::MatchingType,
                ]
            }
            FuncKind::UserDefined => todo!(),
        }
    }
}

impl Display for ArgConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArgConstraint::DynType => write!(f, "DynType"),
            ArgConstraint::MatchingType => write!(f, "MatchingType"),
            ArgConstraint::Numeric => write!(f, "Numeric"),
            ArgConstraint::Integer => write!(f, "Integer"),
            ArgConstraint::Float => write!(f, "Float"),
            ArgConstraint::Str => write!(f, "str"),
            // I think this is fine?
            ArgConstraint::ParamCount(count) => write!(f, "{count} parameter(s)"),
        }
    }
}

// Can't really use AstId since it's not an ExprId and it would pretty much be a guess as to what
// typeexpr it came from
