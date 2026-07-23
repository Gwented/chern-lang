use super::helpers::*;
use crate::script_compiler::{
    DIRECTIVE_BIN_IDX, DIRECTIVE_HEX_IDX, DIRECTIVE_IGNORE_IDX, DIRECTIVE_OCTAL_IDX,
    DIRECTIVE_SCIENT_IDX, DIRECTIVE_UNICODE_IDX, DIRECTIVE_WARN_IDX, ScriptCompiler,
    directive_to_id,
};
use chrn_utils::{id_types::DirectiveId, intern, intern::Intern};
use lang::directives::{BUILTIN_DIRECTIVE_STRS, Directive, TypeDirective};
//NOTE: Only the idices matching test will exist if something ilke #lang(C) is added since those
//types of directives wouldn't be pre-registered, and would more so had inners that may carry known
//identifiers.

// -- ADDING A NEW DIRECTIVE --
// Tests that enforce the requirements in compilation/README.md.

/// Every string in BUILTIN_DIRECTIVE_STRS must be preloaded in the interner and
/// recognized by Directive::try_from_interned_str. This covers:
///   - adding the interned string constant (and preloading it)
///   - adding the arm to try_from_interned_str
///   - adding to BUILTIN_DIRECTIVE_STRS
#[test]
fn all_builtin_directive_strs_are_recognized() {
    let interner = Intern::init();
    for &directive_str in BUILTIN_DIRECTIVE_STRS.iter() {
        let interned = interner
            .try_search_str(directive_str)
            .unwrap_or_else(|| panic!("'{directive_str}' not preloaded in interner"));
        assert!(
            Directive::try_from_interned_str(interned).is_some(),
            "'{directive_str}' is in BUILTIN_DIRECTIVE_STRS but try_from_interned_str returns None"
        );
    }
}

/// Number of directives registered by load_directives() must match BUILTIN_DIRECTIVE_STRS.
#[test]
fn directive_count_consistency() {
    let module = Module::new(
        InternedId::new(0),
        ModuleState::Loaded,
        ModuleId::new(0),
        vec![],
        None,
    );
    let compiler = ScriptCompiler::init(None, Arena::<Module, ModuleId>::from(vec![module]));
    assert_eq!(
        compiler.directives.len(),
        BUILTIN_DIRECTIVE_STRS.len(),
        "load_directives() registered {} directives but BUILTIN_DIRECTIVE_STRS has {}",
        compiler.directives.len(),
        BUILTIN_DIRECTIVE_STRS.len(),
    );
}

/// Each DIRECTIVE_*_IDX constant must point at the expected directive in the compiler's
/// directive arena after init (covers pre-registered indices step).
#[test]
fn pre_registered_directive_indices_match() {
    let module = Module::new(
        InternedId::new(0),
        ModuleState::Loaded,
        ModuleId::new(0),
        vec![],
        None,
    );
    let compiler = ScriptCompiler::init(None, Arena::<Module, ModuleId>::from(vec![module]));
    assert_eq!(
        compiler.directives[DirectiveId::new(DIRECTIVE_WARN_IDX as u32)],
        Directive::Warn,
    );
    assert_eq!(
        compiler.directives[DirectiveId::new(DIRECTIVE_IGNORE_IDX as u32)],
        Directive::Ignore,
    );
    assert_eq!(
        compiler.directives[DirectiveId::new(DIRECTIVE_SCIENT_IDX as u32)],
        Directive::Type(TypeDirective::Scient),
    );
    assert_eq!(
        compiler.directives[DirectiveId::new(DIRECTIVE_HEX_IDX as u32)],
        Directive::Type(TypeDirective::Hex),
    );
    assert_eq!(
        compiler.directives[DirectiveId::new(DIRECTIVE_BIN_IDX as u32)],
        Directive::Type(TypeDirective::Bin),
    );
    assert_eq!(
        compiler.directives[DirectiveId::new(DIRECTIVE_OCTAL_IDX as u32)],
        Directive::Type(TypeDirective::Octal),
    );
    assert_eq!(
        compiler.directives[DirectiveId::new(DIRECTIVE_UNICODE_IDX as u32)],
        Directive::Type(TypeDirective::Unicode),
    );
}

/// directive_to_id must return a unique ID for every variant without panicking.
#[test]
fn directive_to_id_covers_all_variants() {
    let directives = [
        Directive::Warn,
        Directive::Ignore,
        Directive::Type(TypeDirective::Scient),
        Directive::Type(TypeDirective::Hex),
        Directive::Type(TypeDirective::Bin),
        Directive::Type(TypeDirective::Octal),
        Directive::Type(TypeDirective::Unicode),
    ];
    let ids: std::collections::HashSet<_> =
        directives.iter().map(|d| directive_to_id(d).id).collect();
    assert_eq!(
        ids.len(),
        directives.len(),
        "directive_to_id must return a unique ID for each variant",
    );
}

/// All directive interned IDs must fall within the INTERNER_PRELOAD_SIZE range.
/// If a new directive is added its interned ID must be < INTERNER_PRELOAD_SIZE,
/// otherwise INTERNER_PRELOAD_SIZE must be updated.
#[test]
fn interner_preload_size_covers_all_directives() {
    let interner = Intern::init();
    for &s in BUILTIN_DIRECTIVE_STRS.iter() {
        let id = interner.try_search_str(s).unwrap().id;
        assert!(
            (id as usize) < intern::INTERNER_PRELOAD_SIZE,
            "Directive '{s}' has interned ID {id} but INTERNER_PRELOAD_SIZE is {} — \
             update INTERNER_PRELOAD_SIZE to cover the new constant",
            intern::INTERNER_PRELOAD_SIZE,
        );
    }
}
