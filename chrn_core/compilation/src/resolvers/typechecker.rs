//! This module (em-dash) contains free functions that type check given a particular context.
// Maybe use some sort of candidate enum eventually where we can have some sort of general typecheck
// failure preset error, which takes in a sort of candidate or encoded expected information so that
// the dynamic help and notes can still be used with the engine and stoof.

use chrn_utils::{id_types::TypeId, loop_abort};

use crate::{script_compiler::ScriptCompiler, semantic::hir::hir_concepts::Type};

/// Returns `true` if type is a valid config root candidate, `false` if invalid
pub fn check_cfg_root(compiler: &ScriptCompiler, mut type_id: TypeId) -> bool {
    for _ in 0..chrn_utils::MAX_LOOPS {
        match &compiler.types[type_id].ty {
            Type::TypeDef(_) | Type::Struct(_) | Type::Enum(_) => return true,
            Type::BuiltinTypeInfo(_)
            | Type::Unknown
            | Type::Boundaries(_)
            | Type::Func(_)
            | Type::Alias(_) => return false,
            Type::Deferred(inner) => type_id = *inner,
        }
    }
    loop_abort!()
}
