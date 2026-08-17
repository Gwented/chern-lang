//! Compiler generated compiler-specific helpers

use chrn_utils::intern;
use lang::types::{boundaries::TypeBoundaryFlags, builtins::BuiltinType};

use crate::{
    constraints::ArgConstraint,
    script_compiler::{
        CORE_BIGFLOAT, CORE_BIGINT, CORE_BOOL, CORE_CHAR, CORE_F16, CORE_F32, CORE_F64, CORE_F128,
        CORE_I8, CORE_I16, CORE_I32, CORE_I64, CORE_I128, CORE_NIL, CORE_RUNTIME, CORE_SIZED,
        CORE_STR, CORE_U8, CORE_U16, CORE_U32, CORE_U64, CORE_U128, CORE_UNKNOWN, CORE_UNSIZED,
    },
    semantic::hir::hir_symbols::FuncKind,
};

/// Every core builtin type, paired with its interned name and the `TypeId` it must have.
/// `load_core_types` loads them sequentially.
pub static CORE_BUILTIN_TYPES_DATASET: [(u32, BuiltinType, u32); CORE_UNKNOWN as usize] = [
    (intern::INTERNED_I8, BuiltinType::I8, CORE_I8),
    (intern::INTERNED_U8, BuiltinType::U8, CORE_U8),
    (intern::INTERNED_I16, BuiltinType::I16, CORE_I16),
    (intern::INTERNED_U16, BuiltinType::U16, CORE_U16),
    (intern::INTERNED_F16, BuiltinType::F16, CORE_F16),
    (intern::INTERNED_I32, BuiltinType::I32, CORE_I32),
    (intern::INTERNED_U32, BuiltinType::U32, CORE_U32),
    (intern::INTERNED_F32, BuiltinType::F32, CORE_F32),
    (intern::INTERNED_I64, BuiltinType::I64, CORE_I64),
    (intern::INTERNED_U64, BuiltinType::U64, CORE_U64),
    (intern::INTERNED_F64, BuiltinType::F64, CORE_F64),
    (intern::INTERNED_I128, BuiltinType::I128, CORE_I128),
    (intern::INTERNED_U128, BuiltinType::U128, CORE_U128),
    (intern::INTERNED_F128, BuiltinType::F128, CORE_F128),
    (intern::INTERNED_SIZED, BuiltinType::Sized, CORE_SIZED),
    (intern::INTERNED_UNSIZED, BuiltinType::Unsized, CORE_UNSIZED),
    (intern::INTERNED_STR, BuiltinType::Str, CORE_STR),
    (intern::INTERNED_CHAR, BuiltinType::Char, CORE_CHAR),
    (intern::INTERNED_NIL, BuiltinType::Nil, CORE_NIL),
    (intern::INTERNED_BOOL, BuiltinType::Bool, CORE_BOOL),
    (intern::INTERNED_BIGINT, BuiltinType::BigInt, CORE_BIGINT),
    (
        intern::INTERNED_BIGFLOAT,
        BuiltinType::BigFloat,
        CORE_BIGFLOAT,
    ),
    (intern::INTERNED_RUNTIME, BuiltinType::Runtime, CORE_RUNTIME),
];

/// Every core boundary type, paired with its interned name. Loaded after `CORE_BUILTIN_TYPES` and
/// the unknown type, so these have no `CORE_*` constants.
pub static CORE_BOUNDARIES_DATASET: [(u32, TypeBoundaryFlags); 11] = [
    (intern::INTERNED_RANGED, TypeBoundaryFlags::RANGED),
    (
        intern::INTERNED_CHARACTER_MAPPABLE,
        TypeBoundaryFlags::CHARACTER_MAPPABLE,
    ),
    (intern::INTERNED_COLLECTION, TypeBoundaryFlags::COLLECTION),
    (intern::INTERNED_HAS_LEN, TypeBoundaryFlags::HAS_LEN),
    (intern::INTERNED_INTEGER, TypeBoundaryFlags::INTEGER),
    (intern::INTERNED_NUMERIC, TypeBoundaryFlags::NUMERIC),
    (
        intern::INTERNED_SIGNED_INTEGER,
        TypeBoundaryFlags::SIGNED_INTEGER,
    ),
    (
        intern::INTERNED_UNSIGNED_INTEGER,
        TypeBoundaryFlags::UNSIGNED_INTEGER,
    ),
    (intern::INTERNED_FLOAT, TypeBoundaryFlags::FLOAT),
    (intern::INTERNED_ORDERED, TypeBoundaryFlags::ORDERED),
    (intern::INTERNED_COMPARABLE, TypeBoundaryFlags::COMPARABLE),
];

/// A single core function or predicate. Mirrors `FuncDef`, minus the ids the compiler assigns
/// while loading.
pub struct CoreFunc {
    /// Interned index of the function's name
    pub name: u32,
    pub kind: FuncKind,
    pub is_callable: bool,
    pub type_constraints: TypeBoundaryFlags,
    pub arg_constraints: &'static [ArgConstraint],
    pub affects_type_constraint: bool,
    /// `TypeId` of the return type, which must already be loaded as a core type
    pub ret_type: u32,
}

impl CoreFunc {
    const fn new(
        name: u32,
        kind: FuncKind,
        is_callable: bool,
        type_constraints: TypeBoundaryFlags,
        arg_constraints: &'static [ArgConstraint],
        affects_type_constraint: bool,
        ret_type: u32,
    ) -> CoreFunc {
        CoreFunc {
            name,
            kind,
            is_callable,
            type_constraints,
            arg_constraints,
            affects_type_constraint,
            ret_type,
        }
    }
}

/// Every core function and predicate. `load_core_funcs` loads them sequentially, after
/// `CORE_BUILTIN_TYPES_DATASET`, the unknown type, and `CORE_BOUNDARIES_DATASET`.
pub static CORE_FUNCS_DATASET: [CoreFunc; 7] = [
    CoreFunc::new(
        intern::INTERNED_IS_EMPTY,
        FuncKind::IsEmpty,
        false,
        TypeBoundaryFlags::COLLECTION,
        &[ArgConstraint::ArgCount(0)],
        true,
        CORE_BOOL,
    ),
    CoreFunc::new(
        intern::INTERNED_IS_WHITESPACE,
        FuncKind::IsWhitespace,
        false,
        TypeBoundaryFlags::CHARACTER_MAPPABLE,
        &[ArgConstraint::ArgCount(0), ArgConstraint::CharacterMappable],
        true,
        CORE_BOOL,
    ),
    CoreFunc::new(
        intern::INTERNED_CONTAINS,
        FuncKind::Contains,
        true,
        TypeBoundaryFlags::CHARACTER_MAPPABLE,
        &[ArgConstraint::ArgCount(1), ArgConstraint::CharacterMappable],
        true,
        CORE_BOOL,
    ),
    CoreFunc::new(
        intern::INTERNED_STARTSW,
        FuncKind::StartsW,
        true,
        TypeBoundaryFlags::CHARACTER_MAPPABLE,
        &[ArgConstraint::ArgCount(1), ArgConstraint::CharacterMappable],
        true,
        CORE_BOOL,
    ),
    CoreFunc::new(
        intern::INTERNED_ENDSW,
        FuncKind::EndsW,
        true,
        TypeBoundaryFlags::CHARACTER_MAPPABLE,
        &[ArgConstraint::ArgCount(1), ArgConstraint::CharacterMappable],
        true,
        CORE_BOOL,
    ),
    CoreFunc::new(
        intern::INTERNED_RANGE,
        FuncKind::Range,
        true,
        TypeBoundaryFlags::RANGED,
        &[
            ArgConstraint::ArgCount(2),
            ArgConstraint::Numeric,
            ArgConstraint::MatchingArgumentTypes,
            ArgConstraint::SameTypeAsSelf,
        ],
        true,
        CORE_BOOL,
    ),
    CoreFunc::new(
        intern::INTERNED_EQUALS,
        FuncKind::Equals,
        true,
        TypeBoundaryFlags::COMPARABLE,
        &[
            ArgConstraint::ArgCount(1),
            ArgConstraint::Comparable,
            ArgConstraint::SameTypeAsSelf,
        ],
        true,
        CORE_BOOL,
    ),
];
