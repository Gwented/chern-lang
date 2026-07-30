use chrn_utils::{
    id_types::{ModuleId, SymbolId},
    source_map::source_region::SourceRegion,
};

use crate::{parser::ast::ast_concepts::AstInfo, semantic::compilation_unit::CompilationUnit};

/// Represents an environment before any form of symbols regarding compilation have been created.
/// This is only used for `NameResolver` right now but is still the general environmental state of
/// any resolver before symbols.
///
/// This exists so that there is an explicit structure displaying what is and isn't swapped out.
pub struct RegistrationEnv<'a> {
    // Should be ids
    pub(crate) ast_info: &'a AstInfo,
    pub(crate) region: &'a SourceRegion,
    pub(crate) current_mod: ModuleId,
}

impl RegistrationEnv<'_> {
    pub fn new<'a>(
        ast_info: &'a AstInfo,
        region: &'a SourceRegion,
        current_mod: ModuleId,
    ) -> RegistrationEnv<'a> {
        RegistrationEnv {
            ast_info,
            region,
            current_mod,
        }
    }
}

/// Representing the current environment of every resolver stage past `RegistrationEnv` which is
/// denoted by it's holding of `compilation_syms`
///
/// This exists so that there is an explicit structure displaying what is and isn't swapped out.
#[derive(Debug, Clone)]
pub struct ResolverEnv<'a> {
    // Should be ids
    pub(crate) ast_info: &'a AstInfo,
    pub(crate) region: &'a SourceRegion,
    pub(crate) current_mod: ModuleId,
    pub(crate) compilation_syms: &'a [CompilationUnit],
}

impl ResolverEnv<'_> {
    pub fn new<'a>(
        ast_info: &'a AstInfo,
        region: &'a SourceRegion,
        current_mod: ModuleId,
        compilation_syms: &'a [CompilationUnit],
    ) -> ResolverEnv<'a> {
        ResolverEnv {
            ast_info,
            region,
            current_mod,
            compilation_syms,
        }
    }
}
