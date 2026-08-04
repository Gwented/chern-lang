//! This module (em-dash) contains free functions that type check given a particular context.
// Maybe use some sort of candidate enum eventually where we can have some sort of general typecheck
// failure preset error, which takes in a sort of candidate or encoded expected information so that
// the dynamic help and notes can still be used with the engine and stoof.

use chrn_utils::{arena::Arena, id_types::TypeId, loop_abort};

use crate::{
    semantic::hir::hir_concepts::{Type, TypeInfo},
    walk_type_id_deferred,
};

//What about just a general walk deferred function that prevents this same code from being written
//everywhere
/// Fields and variants encode the same type semantics so this is shared.
///
/// Returns `true` if the type is a valid field or variant candidate, `false` if invalid
pub fn check_field_or_variant(types: &Arena<TypeInfo, TypeId>, mut type_id: TypeId) -> bool {
    let checked = walk_type_id_deferred!(types, type_id);
    match &types[checked.inner].ty {
        Type::Struct(_) | Type::Enum(_) | Type::BuiltinTypeInfo(_) => true,
        Type::Unknown | Type::Boundaries(_) | Type::Alias(_) | Type::TypeDef(_) | Type::Func(_) => {
            false
        }
        Type::Deferred(_) => unreachable!(),
    }
}

/// Returns `true` if the type is a valid config root candidate, `false` if invalid
pub fn check_cfg_root(types: &Arena<TypeInfo, TypeId>, mut type_id: TypeId) -> bool {
    let checked = walk_type_id_deferred!(types, type_id);
    match &types[checked.inner].ty {
        Type::TypeDef(_) | Type::Struct(_) | Type::Enum(_) => return true,
        Type::BuiltinTypeInfo(_)
        | Type::Unknown
        | Type::Boundaries(_)
        | Type::Func(_)
        | Type::Alias(_) => return false,
        Type::Deferred(_) => unreachable!(),
    }
}
