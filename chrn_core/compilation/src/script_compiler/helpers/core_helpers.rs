//! Compiler generated compiler-specific helpers

use chrn_utils::{id_types::InternedId, intern};
use lang::types::{
    boundaries::TypeBoundaryFlags,
    builtins::{BuiltinType, BuiltinTypeKind},
};

use crate::{
    constraints::ArgConstraint,
    lookup::scopes::scopes_concepts::ScopeType,
    script_compiler::{
        compiler_constants::{
            self, CORE_BOOL, CORE_F32, CORE_F64, CORE_F128, CORE_I8, CORE_I16, CORE_I32, CORE_I64,
            CORE_I128, CORE_SIZED, CORE_U8, CORE_U16, CORE_U32, CORE_U64, CORE_U128, CORE_UNKNOWN,
            CORE_UNSIZED,
        },
        helpers::instantiation_symbols::{
            InstantiationSymbolBase, InstantiationSymbolKind, InstantiationVariable, InstiationType,
        },
    },
    semantic::hir::hir_symbols::{FuncKind, SymbolOrigin},
};

use super::instantiation_symbols::InstiationValue;
//TEST:

static NAMESPACE_I8: [InstantiationSymbolBase; 2] = [
    new_max(InstantiationVariable::new(
        InstiationType::BuiltinType(BuiltinType::I8),
        InstiationValue::I64(i8::MAX as i64),
    )),
    new_min(InstantiationVariable::new(
        InstiationType::BuiltinType(BuiltinType::I8),
        InstiationValue::I64(i8::MIN as i64),
    )),
];

static NAMESPACE_U8: [InstantiationSymbolBase; 2] = [
    new_max(InstantiationVariable::new(
        InstiationType::BuiltinType(BuiltinType::U8),
        InstiationValue::I64(u8::MAX as i64),
    )),
    new_min(InstantiationVariable::new(
        InstiationType::BuiltinType(BuiltinType::U8),
        InstiationValue::I64(u8::MIN as i64),
    )),
];

static NAMESPACE_I16: [InstantiationSymbolBase; 2] = [
    new_max(InstantiationVariable::new(
        InstiationType::BuiltinType(BuiltinType::I16),
        InstiationValue::I64(i16::MAX as i64),
    )),
    new_min(InstantiationVariable::new(
        InstiationType::BuiltinType(BuiltinType::I16),
        InstiationValue::I64(i16::MIN as i64),
    )),
];

static NAMESPACE_U16: [InstantiationSymbolBase; 2] = [
    new_max(InstantiationVariable::new(
        InstiationType::BuiltinType(BuiltinType::U16),
        InstiationValue::I64(u16::MAX as i64),
    )),
    new_min(InstantiationVariable::new(
        InstiationType::BuiltinType(BuiltinType::U16),
        InstiationValue::I64(u16::MIN as i64),
    )),
];

//NOTE: Rust has no stable `f16`, so the IEEE-754 binary16 bounds are spelled out. Both are exact
//in `f64`.
static NAMESPACE_F16: [InstantiationSymbolBase; 2] = [
    new_max(InstantiationVariable::new(
        InstiationType::BuiltinType(BuiltinType::F16),
        InstiationValue::F64(65504.0),
    )),
    new_min(InstantiationVariable::new(
        InstiationType::BuiltinType(BuiltinType::F16),
        InstiationValue::F64(-65504.0),
    )),
];

static NAMESPACE_I32: [InstantiationSymbolBase; 2] = [
    new_max(InstantiationVariable::new(
        InstiationType::BuiltinType(BuiltinType::I32),
        InstiationValue::I64(i32::MAX as i64),
    )),
    new_min(InstantiationVariable::new(
        InstiationType::BuiltinType(BuiltinType::I32),
        InstiationValue::I64(i32::MIN as i64),
    )),
];

static NAMESPACE_U32: [InstantiationSymbolBase; 2] = [
    new_max(InstantiationVariable::new(
        InstiationType::BuiltinType(BuiltinType::U32),
        InstiationValue::I64(u32::MAX as i64),
    )),
    new_min(InstantiationVariable::new(
        InstiationType::BuiltinType(BuiltinType::U32),
        InstiationValue::I64(u32::MIN as i64),
    )),
];

static NAMESPACE_F32: [InstantiationSymbolBase; 2] = [
    new_max(InstantiationVariable::new(
        InstiationType::BuiltinType(BuiltinType::F32),
        InstiationValue::F64(f32::MAX as f64),
    )),
    new_min(InstantiationVariable::new(
        InstiationType::BuiltinType(BuiltinType::F32),
        InstiationValue::F64(f32::MIN as f64),
    )),
];

static NAMESPACE_I64: [InstantiationSymbolBase; 2] = [
    new_max(InstantiationVariable::new(
        InstiationType::BuiltinType(BuiltinType::I64),
        InstiationValue::I64(i64::MAX),
    )),
    new_min(InstantiationVariable::new(
        InstiationType::BuiltinType(BuiltinType::I64),
        InstiationValue::I64(i64::MIN),
    )),
];

static NAMESPACE_F64: [InstantiationSymbolBase; 2] = [
    new_max(InstantiationVariable::new(
        InstiationType::BuiltinType(BuiltinType::F64),
        InstiationValue::F64(f64::MAX),
    )),
    new_min(InstantiationVariable::new(
        InstiationType::BuiltinType(BuiltinType::F64),
        InstiationValue::F64(f64::MIN),
    )),
];

//TODO: `u64`, `i128`, `u128` and `f128` bounds do not fit `InstiationValue`, which only carries
//`I64` and `F64`. `sized`/`unsized` are pointer-sized, so their bounds belong to the target rather
//than the host. All six stay empty until the value system covers them.

// Will just be [[]] like the ns entries
/// Every core builtin type, paired with its interned name and the `TypeId` it must have.
/// `load_core_types` loads them sequentially.
pub static CORE_BUILTIN_TYPES_DATASET: [(u32, BuiltinType, &'static [InstantiationSymbolBase]);
    CORE_UNKNOWN as usize] = [
    (intern::INTERNED_I8, BuiltinType::I8, &NAMESPACE_I8),
    (intern::INTERNED_U8, BuiltinType::U8, &NAMESPACE_U8),
    (intern::INTERNED_I16, BuiltinType::I16, &NAMESPACE_I16),
    (intern::INTERNED_U16, BuiltinType::U16, &NAMESPACE_U16),
    (intern::INTERNED_F16, BuiltinType::F16, &NAMESPACE_F16),
    (intern::INTERNED_I32, BuiltinType::I32, &NAMESPACE_I32),
    (intern::INTERNED_U32, BuiltinType::U32, &NAMESPACE_U32),
    (intern::INTERNED_F32, BuiltinType::F32, &NAMESPACE_F32),
    (intern::INTERNED_I64, BuiltinType::I64, &NAMESPACE_I64),
    // -- Eveneutally faojjkaj --
    (intern::INTERNED_U64, BuiltinType::U64, &[]),
    // -- Eveneutally faojjkaj --
    (intern::INTERNED_F64, BuiltinType::F64, &NAMESPACE_F64),
    // -- Eveneutally faojjkaj --
    (intern::INTERNED_I128, BuiltinType::I128, &[]),
    (intern::INTERNED_U128, BuiltinType::U128, &[]),
    (intern::INTERNED_F128, BuiltinType::F128, &[]),
    (intern::INTERNED_SIZED, BuiltinType::Sized, &[]),
    (intern::INTERNED_UNSIZED, BuiltinType::Unsized, &[]),
    // -- Eveneutally faojjkaj --
    // Unrelated but this should have .len() later
    (intern::INTERNED_STR, BuiltinType::Str, &[]),
    (intern::INTERNED_CHAR, BuiltinType::Char, &[]),
    (intern::INTERNED_NIL, BuiltinType::Nil, &[]),
    (intern::INTERNED_BOOL, BuiltinType::Bool, &[]),
    (intern::INTERNED_BIGINT, BuiltinType::BigInt, &[]),
    (intern::INTERNED_BIGFLOAT, BuiltinType::BigFloat, &[]),
    (intern::INTERNED_RUNTIME, BuiltinType::Runtime, &[]),
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

/// What a set of instantiation bases contributes to the compiler's arenas when registered.
#[derive(Default)]
pub struct InstantiationReservations {
    pub symbols: usize,
    pub scopes: usize,
    /// One `VarDef`, one `ResolvedExpr`, and one `ValueInfo` each.
    pub variables: usize,
}

impl InstantiationReservations {
    fn merge(&mut self, other: InstantiationReservations) {
        self.symbols += other.symbols;
        self.scopes += other.scopes;
        self.variables += other.variables;
    }
}

/// Counts what `register_instantiation_bases` pushes for `bases`. Every base is one symbol, a
/// namespace additionally owns a scope and whatever it holds.
pub fn count_instantiation_bases(bases: &[InstantiationSymbolBase]) -> InstantiationReservations {
    let mut counts = InstantiationReservations::default();

    for base in bases {
        counts.symbols += 1;
        match &base.kind {
            InstantiationSymbolKind::Namespace(inner) => {
                counts.scopes += 1;
                counts.merge(count_instantiation_bases(inner));
            }
            InstantiationSymbolKind::Variable(_) => counts.variables += 1,
            InstantiationSymbolKind::ExternType(_) => (),
        }
    }

    counts
}

/// Counts what the intrinsic namespaces of `CORE_BUILTIN_TYPES_DATASET` push, such as the `i8::MAX`
/// symbol and the scope holding it. Every non-empty namespace owns a scope on its built-in.
pub fn core_instantiation_reservations() -> InstantiationReservations {
    let mut counts = InstantiationReservations::default();

    for (_, _, ns) in &CORE_BUILTIN_TYPES_DATASET {
        if ns.is_empty() {
            continue;
        }

        counts.scopes += 1;
        counts.merge(count_instantiation_bases(ns));
    }

    counts
}

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

//TEST: This seems a little odd

const MIN_IDENT: InternedId = InternedId::new(intern::INTERNED_MIN_UPPER);
const MAX_IDENT: InternedId = InternedId::new(intern::INTERNED_MAX_UPPER);
const NUMERIC_SYM_ORIGIN: SymbolOrigin = SymbolOrigin::Compiler;
const NUMERIC_SCOPE_ORIGIN: ScopeType = ScopeType::Core;
const NUMERIC_IS_PRIV: bool = false;

const fn new_max(var: InstantiationVariable) -> InstantiationSymbolBase {
    InstantiationSymbolBase::new(
        MAX_IDENT,
        NUMERIC_SYM_ORIGIN,
        NUMERIC_SCOPE_ORIGIN,
        NUMERIC_IS_PRIV,
        InstantiationSymbolKind::Variable(var),
    )
}
const fn new_min(var: InstantiationVariable) -> InstantiationSymbolBase {
    InstantiationSymbolBase::new(
        MIN_IDENT,
        NUMERIC_SYM_ORIGIN,
        NUMERIC_SCOPE_ORIGIN,
        NUMERIC_IS_PRIV,
        InstantiationSymbolKind::Variable(var),
    )
}

//// Contains `Numeric` type expectations of implementers. Strictly compile-time.
//// The intent is to go to the target index, then insert the `Numeric` constants
// pub static CORE_NUMERIC_TARGETS: [NumericTarget; 1] = [
//     NumericTarget::new(
//         CORE_I8,
//         new_max(InstantiationVariable::new(
//             InstiationType::BuiltinType(BuiltinType::I8),
//             InstiationValue::I64(i8::MAX as i64),
//         )),
//         new_min(InstantiationVariable::new(
//             InstiationType::BuiltinType(BuiltinType::I8),
//             InstiationValue::I64(i8::MIN as i64),
//         )),
//     ),
// (CORE_U8, BuiltinType::U8),
// (CORE_I16, BuiltinType::I16),
// (CORE_U16, BuiltinType::U16),
// // (intern::INTERNED_F16, BuiltinType::F16),
// (CORE_I32, BuiltinType::I32),
// (CORE_U32, BuiltinType::U32),
// (CORE_F32, BuiltinType::F32),
// (CORE_I64, BuiltinType::I64),
// (CORE_U64, BuiltinType::U64),
// (CORE_F64, BuiltinType::F64),
// (CORE_I128, BuiltinType::I128),
// (CORE_U128, BuiltinType::U128),
// (CORE_F128, BuiltinType::F128),
// (CORE_SIZED, BuiltinType::Sized),
// (CORE_UNSIZED, BuiltinType::Unsized),
// ];
