// use chrn_utils::{id_types::ModuleId, source_map::source_region::SourceRegion};
//
// use crate::parser::ast::AstInfo;
//
// /// Struct representing the current environment of `TypeResolver` which is designed to be swapped
// /// out.
// ///
// /// This exists so that there is an explicit structure displaying what is and isn't swapped out.
// pub struct TypeResolverEnv<'a> {
//     pub(super) ast_info: &'a AstInfo,
//     pub(super) region: &'a SourceRegion,
//     pub(super) current_mod: ModuleId,
// }
//
// impl TypeResolverEnv<'_> {
//     pub fn new<'a>(
//         ast_info: &'a AstInfo,
//         current_region: &'a SourceRegion,
//         current_mod: ModuleId,
//     ) -> TypeResolverEnv<'a> {
//         TypeResolverEnv {
//             ast_info,
//             region: current_region,
//             current_mod,
//         }
//     }
// }
