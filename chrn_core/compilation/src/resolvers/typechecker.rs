//! This module (em-dash) contains free functions that type check given a particular context.
// Maybe use some sort of candidate enum eventually where we can have some sort of general typecheck
// failure preset error, which takes in a sort of candidate or encoded expected information so that
// the dynamic help and notes can still be used with the engine and stoof.

use chrn_utils::{
    arena::Arena,
    id_types::{SymbolId, TypeId},
};

use crate::{
    lookup::scopes::scopes_concepts::{AssociatedScopeKind, ScopeType},
    script_compiler::ScriptCompiler,
    semantic::hir::{
        hir_concepts::{Type, TypeInfo},
        hir_symbols::{Symbol, SymbolKind, SymbolKindFlat},
    },
    walk_type_id_deferred,
};
//TODO: Typechecker helpers?

// TEST: Not done yet
// Rename to lost and found?
///
pub fn is_expected_sym(
    syms: &Arena<Symbol, SymbolId>,
    expected: SymbolKindFlat,
    sym_id: SymbolId,
) -> bool {
    syms[sym_id].kind.to_flat() == expected
}

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
pub fn check_cfg_root(compiler: &ScriptCompiler, sym_id: SymbolId) -> bool {
    let sym = &compiler.syms[sym_id];
    match sym.kind {
        SymbolKind::Type(mut type_id) => {
            let checked = walk_type_id_deferred!(&compiler.types, type_id);
            match &compiler.types[checked.inner].ty {
                Type::TypeDef(_) | Type::Struct(_) | Type::Enum(_) => return true,
                Type::BuiltinTypeInfo(_)
                | Type::Unknown
                | Type::Boundaries(_)
                | Type::Func(_)
                | Type::Alias(_) => return false,
                Type::Deferred(_) => unreachable!(),
            }
        }
        // Can only accept scope, needs to prevent "for module {}" from being possible as a root
        SymbolKind::Namespace
            if matches!(sym.associated_scope, Some(AssociatedScopeKind::Scope(_))) =>
        {
            true
        }
        SymbolKind::Namespace
        | SymbolKind::Variable(_)
        | SymbolKind::Directive(_)
        | SymbolKind::ExternType(_) => false,
    }
}

// This doesn't really have use because config members are required to search for other intrinsics
// anyways so a failure to find a symbol is the only type of failure. As of right now.
/// Returns `true` if the type is a valid override config member candidate, `false` if invalid
pub fn check_cfg_memb_override(compiler: &ScriptCompiler, sym_id: SymbolId) -> bool {
    let sym = &compiler.syms[sym_id];
    match sym.kind {
        // Only override section symbols can access a namespace in it's config root.
        SymbolKind::Namespace => true,
        SymbolKind::Variable(_)
        | SymbolKind::Type(_)
        | SymbolKind::Directive(_)
        | SymbolKind::ExternType(_) => false,
    }
}

// Not used either...
/// Returns `true` if the type is a valid complex config member candidate, `false` if invalid
pub fn check_cfg_memb_complex(compiler: &ScriptCompiler, sym_id: SymbolId) -> bool {
    let sym = &compiler.syms[sym_id];
    match sym.kind {
        SymbolKind::Type(mut type_id) => {
            let checked = walk_type_id_deferred!(&compiler.types, type_id);
            match &compiler.types[checked.inner].ty {
                Type::TypeDef(_) | Type::Struct(_) | Type::Enum(_) => return true,
                // Brain failing here
                Type::BuiltinTypeInfo(_)
                | Type::Unknown
                | Type::Boundaries(_)
                | Type::Func(_)
                | Type::Alias(_) => return false,
                Type::Deferred(_) => unreachable!(),
            }
        }
        // Only override section symbols can access a namespace in it's config root.
        SymbolKind::Namespace => true,
        SymbolKind::Variable(_) | SymbolKind::Directive(_) | SymbolKind::ExternType(_) => false,
    }
}
