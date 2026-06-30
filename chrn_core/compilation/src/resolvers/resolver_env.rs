use chrn_utils::{id_types::ModuleId, source_map::source_region::SourceRegion};

use crate::parser::ast::ast_concepts::AstInfo;

/// Struct representing the current environment of `TypeResolver` which is designed to be swapped
/// out.
///
/// This exists so that there is an explicit structure displaying what is and isn't swapped out.
pub struct ResolverEnv<'a> {
    // Should be ids
    pub(crate) ast_info: &'a AstInfo,
    pub(crate) region: &'a SourceRegion,
    pub(crate) current_mod: ModuleId,
}

impl ResolverEnv<'_> {
    pub fn new<'a>(
        ast_info: &'a AstInfo,
        region: &'a SourceRegion,
        current_mod: ModuleId,
    ) -> ResolverEnv<'a> {
        ResolverEnv {
            ast_info,
            region,
            current_mod,
        }
    }
}
