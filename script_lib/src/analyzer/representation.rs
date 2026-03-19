use std::collections::HashMap;

use common::{
    builtins::BuiltinType,
    symbols::{AstId, Cond, FuncId, InnerArgs, NameId, SpannedInnerArgs, TypedId},
};

// What is a drop? I am new to thinking i have never thought before what is RAII
// is that a gui framework
pub struct Table {
    //FIXME: MUCH RATHER USE IF LET
    //Maybe
    pub(super) sym_table: HashMap<NameId, TypedId>,
    pub(super) typedefs: Vec<TypeDefRepre>,
    pub(super) structs: Vec<StructRepre>,
    pub(super) funcs: Vec<FuncRepre>,
    pub(super) enums: Vec<EnumRepre>,
    pub(super) builtin_types: Vec<BuiltinType>,
}

impl Table {
    pub fn new() -> Table {
        Table {
            sym_table: HashMap::new(),
            typedefs: Vec::new(),
            structs: Vec::new(),
            funcs: Vec::new(),
            enums: Vec::new(),
            builtin_types: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub(super) struct StructRepre {
    pub(super) name_id: NameId,
    pub(super) ast_id: AstId,
    pub(super) fields: Vec<FieldRepre>,
    pub(super) args: Vec<SpannedInnerArgs>,
    pub(super) conds: Vec<Cond>,
}

impl StructRepre {
    pub fn new(name_id: NameId, ast_id: AstId) -> StructRepre {
        StructRepre {
            name_id,
            ast_id,
            fields: Vec::new(),
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
    pub(super) ast_id: AstId,
    pub(super) variants: Vec<VariantRepre>,
    pub(super) args: Vec<InnerArgs>,
    pub(super) conds: Vec<Cond>,
}

impl EnumRepre {
    pub fn new(name_id: NameId, ast_id: AstId) -> EnumRepre {
        EnumRepre {
            name_id,
            ast_id,
            variants: Vec::new(),
            args: Vec::new(),
            conds: Vec::new(),
        }
    }

    //NOTE: I will not use bit masks I will not use bitmasks I will n
    // pub fn supports_arg(&self, arg: InnerArgs) -> bool {
    //     match arg {
    //         InnerArgs::Warn
    //         | InnerArgs::Scientific
    //         | InnerArgs::Hex
    //         | InnerArgs::Binary
    //         | InnerArgs::Octal => true,
    //     }
    // }
}

#[derive(Debug)]
pub struct VariantRepre {
    pub(super) name_id: NameId,
    //WARN: Not because of being a representation but because enum types are nullable
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
    pub(super) ast_id: AstId,
    // This is not nullable but it is invalid, should a more descriptive "TypeState" enum be used
    // or is that not needed?
    pub(super) type_id: Option<TypedId>,
    //TODO: Could make a wrapper for getting the type after resolution so that the code smell is not
    // gone but hidden.
    pub(super) conds: Vec<Cond>,
    pub(super) args: Vec<InnerArgs>,
}

impl TypeDefRepre {
    pub fn new(name_id: NameId, ast_id: AstId) -> TypeDefRepre {
        TypeDefRepre {
            name_id,
            ast_id,
            type_id: None,
            conds: Vec::new(),
            args: Vec::new(),
        }
    }

    pub fn supports_arg(&self, arg: InnerArgs) -> bool {
        match arg {
            InnerArgs::Warn => true,
            // structs, enums, and alias functions cannot have arguments beyond warn or future generic ones that
            // define how the program should react to it's data, rather than literal changes like
            // hex
            _ => {
                //NOTE: A TypeDef cannot point to a TypeDef
                if let Some(type_id) = self.type_id {
                    match type_id {
                        TypedId::Struct(_) | TypedId::Enum(_) | TypedId::Func(_) => return false,
                        // I don't actually think it CAN point to a typedef or function, at all
                        TypedId::TypeDef(_) | TypedId::BuiltinType(_) => return true,
                    }
                }

                return true;
            }
        }
    }
}

#[derive(Debug)]
pub(super) struct FuncRepre {
    pub(super) name_id: NameId,
    pub(super) func_id: FuncId,
    // pub(super) ast_id: AstId,
    pub(super) field: Vec<FuncArgsRepre>,
}

impl FuncRepre {
    pub(super) fn new(
        name_id: NameId,
        func_id: FuncId,
        // ast_id: AstId,
        field: Vec<FuncArgsRepre>,
    ) -> FuncRepre {
        FuncRepre {
            name_id,
            // ast_id,
            func_id,
            field,
        }
    }
}

#[derive(Debug)]
pub(super) enum FuncArgsRepre {
    Integer(i64),
    Float(f64),
    Char(char),
    Str(NameId),
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
