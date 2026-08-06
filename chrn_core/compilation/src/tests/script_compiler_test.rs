use super::helpers::*;

use crate::{
    script_compiler::{CORE_BOUNDARIES, CORE_BUILTIN_TYPES, CORE_UNKNOWN},
    semantic::hir::{
        hir_concepts::{Type, TypeInfo},
        hir_symbols::SymbolKind,
    },
};
use chrn_utils::id_types::TypeId;

/// Builds the core module the same way `ScriptCompiler::init` does, with no user modules
fn core_only_compiler() -> ScriptCompiler {
    ScriptCompiler::init(None, Arena::new())
}

#[test]
fn core_builtin_type_id_alignment() {
    let compiler = core_only_compiler();
    let interner = Intern::init();

    for (interned, builtin_ty, core_id) in CORE_BUILTIN_TYPES.iter().cloned() {
        let type_info: &TypeInfo = &compiler.types[TypeId::new(core_id)];

        let Type::BuiltinTypeInfo(info) = &type_info.ty else {
            panic!(
                "expected builtin type at CORE id {core_id}, got {:?}",
                type_info.ty
            );
        };

        assert_eq!(
            info.ty.kind(),
            builtin_ty.kind(),
            "builtin at CORE id {core_id} does not match the table entry"
        );

        let sym = &compiler.symbols[info.sym_id];
        assert_eq!(
            sym.name_id,
            InternedId::new(interned),
            "symbol name for {} does not match its interned id",
            interner.search_idx(interned as usize)
        );
    }
}

#[test]
fn core_unknown_type_id_alignment() {
    let compiler = core_only_compiler();
    let type_info = &compiler.types[TypeId::new(CORE_UNKNOWN)];

    assert!(
        matches!(type_info.ty, Type::Unknown),
        "expected Type::Unknown at CORE_UNKNOWN, got {:?}",
        type_info.ty
    );
}

#[test]
fn core_boundaries_follow_builtins() {
    let compiler = core_only_compiler();

    // Boundaries are pushed directly after the unknown type
    let first = CORE_UNKNOWN + 1;

    for (idx, (interned, flags)) in CORE_BOUNDARIES.into_iter().enumerate() {
        let type_id = TypeId::new(first + idx as u32);
        let type_info = &compiler.types[type_id];

        let Type::Boundaries(found) = type_info.ty else {
            panic!(
                "expected boundary type at {type_id:?}, got {:?}",
                type_info.ty
            );
        };

        assert_eq!(found, flags, "boundary at {type_id:?} does not match");

        let sym = compiler
            .symbols
            .items
            .iter()
            .find(|sym| sym.name_id == InternedId::new(interned))
            .expect("boundary symbol should exist");

        assert!(
            matches!(sym.kind, SymbolKind::Type(id) if id == type_id),
            "boundary symbol does not point at its type"
        );
    }
}
