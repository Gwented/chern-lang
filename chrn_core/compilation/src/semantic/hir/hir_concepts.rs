// What is a drop? I am new to thinking i have never thought before what is RAII
// is that a gui framework
// Maybe named, global table, program table

use std::collections::HashMap;

use chrn_utils::{
    arena::Arena,
    id_types::{ExprId, MemberId, ModuleId, ScopeId, SpannedContainer, TypeId, ValueId},
    loop_abort,
};
use lang::{
    fmter::{Formattable, Formatted},
    types::{
        boundaries::TypeBoundaryFlags,
        builtins::{BuiltinType, BuiltinTypeKind},
    },
};

use crate::{
    script_compiler::ScriptCompiler,
    semantic::hir::hir_symbols::{AliasDef, EnumDef, FuncDef, StructDef, TypeDef},
    walk_type_id_deferred,
};

// This is kind of just a "concept" though
use chrn_utils::id_types::{AstId, ConfigRootId, DirectiveId, InternedId, SymbolId, VariableId};

// #[derive(Debug)]
// pub struct SectionInfo {
//     pub sections: [Option<SectionHir>; 5],
//     pub compilation_syms: SymbolId,
// }

// Who is this?
#[derive(Debug)]
pub struct Table {
    pub(crate) ast_to_sym: HashMap<AstId, SymbolId>,
    pub(crate) interned_to_sym: HashMap<InternedId, SymbolId>,
}

impl Table {
    pub fn new() -> Table {
        Table {
            ast_to_sym: HashMap::new(),
            interned_to_sym: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub struct TypeInfo {
    pub ty: Type,
    pub owner: ModuleId,
}

impl TypeInfo {
    pub fn new(ty: Type, owner: ModuleId) -> TypeInfo {
        TypeInfo { ty, owner }
    }
}

// Types are not given spans directly since it would over-complicate storing and add a net 12 byte
// increase to all spans. Also, type spanning is entity symbol dependent anyways so it's likely the
// better choice.
//NOTE: Should be in lang?
#[derive(Debug)]
pub enum Type {
    BuiltinTypeInfo(BuiltinTypeInfo),
    Struct(StructDef),
    Enum(EnumDef),
    Func(FuncDef),
    Alias(AliasDef),
    TypeDef(TypeDef),
    Boundaries(TypeBoundaryFlags),
    /// Preserved stable handle so that anything defined before a type was defined can still point
    /// to the correct type which prevents duplicating different definitions.
    Deferred(TypeId),
    Unknown,
}

/// Required metadata for compiler built-in types
#[derive(Debug)]
pub struct BuiltinTypeInfo {
    pub sym_id: SymbolId,
    pub ty: BuiltinType,
}

impl BuiltinTypeInfo {
    pub fn new(sym_id: SymbolId, ty: BuiltinType) -> BuiltinTypeInfo {
        BuiltinTypeInfo { sym_id, ty }
    }
}

impl Type {
    pub fn kind(compiler: &ScriptCompiler, mut type_id: TypeId) -> TypeKind {
        let checked = walk_type_id_deferred!(compiler.types, type_id);
        match &compiler.types[checked.inner].ty {
            Type::BuiltinTypeInfo(builtin_ty) => TypeKind::BuiltinType(builtin_ty.ty.kind()),
            Type::Struct(_) => TypeKind::Struct,
            Type::Enum(_) => TypeKind::Enum,
            Type::Func(_) => TypeKind::Func,
            Type::Alias(_) => TypeKind::Alias,
            Type::TypeDef(_) => TypeKind::TypeDef,
            // This is the only issue since it's not a single Formatted.
            // The next obvious decision should be to do, "Formatted::NumericIntegerRanged", etc.,
            // where we have 4000 variants which
            Type::Boundaries(_) => TypeKind::Boundaries,
            Type::Unknown => TypeKind::Unknown,
            Type::Deferred(_) => unreachable!(),
        }
    }

    /// Gets the `TypeBoundaryFlags` associated with the given `TypeId`
    pub fn boundaries(compiler: &ScriptCompiler, mut type_id: TypeId) -> Option<TypeBoundaryFlags> {
        // Doesn't use walk deferred since typedef itself actually needs to go into it's inner type
        // for it's boundaries
        for _ in 0..chrn_utils::MAX_LOOPS {
            match &compiler.types[type_id].ty {
                Type::BuiltinTypeInfo(builtin_ty) => {
                    return Some(builtin_ty.ty.kind().boundaries());
                }
                // This is the only issue since it's not a single Formatted.
                // The next obvious decision should be to do, "Formatted::NumericIntegerRanged", etc.,
                Type::Struct(_)
                | Type::Enum(_)
                | Type::Func(_)
                | Type::Alias(_)
                | Type::Unknown => return None,
                // where we have 4000 variants which
                Type::Boundaries(boundaries) => return Some(*boundaries),
                Type::TypeDef(type_def) => type_id = type_def.type_id,
                Type::Deferred(inner) => type_id = *inner,
            }
        }
        loop_abort!()
    }

    // so??
    /// The env can't be passed into to_fmt so
    pub fn to_fmt(types: &Arena<TypeInfo, TypeId>, mut type_id: TypeId) -> Formatted {
        let checked = walk_type_id_deferred!(&types, type_id);
        match &types[checked.inner].ty {
            Type::BuiltinTypeInfo(builtin_type) => builtin_type.ty.kind().to_fmt(),
            Type::Struct(struct_def) => struct_def.to_fmt(),
            Type::Enum(enum_def) => enum_def.to_fmt(),
            Type::Func(func_def) => func_def.to_fmt(),
            Type::Alias(alias_def) => alias_def.to_fmt(),
            Type::TypeDef(type_def) => type_def.to_fmt(),
            // This is the only issue since it's not a single Formatted.
            // The next obvious decision should be to do, "Formatted::NumericIntegerRanged", etc.,
            // where we have 4000 variants which
            Type::Boundaries(flags) => Formatted::Boundaries(*flags),
            Type::Unknown => Formatted::Unknown,
            Type::Deferred(_) => unreachable!(),
        }
    }
}

// WE LOST
/// Flat variation of `Type`
pub enum TypeKind {
    BuiltinType(BuiltinTypeKind),
    Struct,
    TypeDef,
    Boundaries,
    Enum,
    Func,
    Alias,
    Unknown,
}
