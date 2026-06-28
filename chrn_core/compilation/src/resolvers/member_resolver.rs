//TEST:
//Seeing if members having a specific stage will help reduce the complexity of type resolution
//which is stacking infinitely (Infinitely as in the infinite sign here -> 🍔)

use chrn_utils::{
    chrn_settings::ChrnSettings,
    id_types::ModuleId,
    intern::Intern,
    source_map::{source_diagnostic::SourceDiagnostic, source_region::SourceRegion},
};

use crate::{
    parser::ast::ast_concepts::{AstInfo, Item},
    resolvers::resolver_env::ResolverEnv,
    script_compiler::ScriptCompiler,
};

pub struct MemberResolver<'a> {
    settings: &'a ChrnSettings,
    interner: &'a Intern,
    compiler: &'a mut ScriptCompiler,
    err_vec: Vec<SourceDiagnostic>,
}

impl MemberResolver<'_> {
    pub fn new<'a>(
        settings: &'a ChrnSettings,
        interner: &'a Intern,
        compiler: &'a mut ScriptCompiler,
    ) -> MemberResolver<'a> {
        MemberResolver {
            settings,
            interner,
            compiler,
            err_vec: Vec::new(),
        }
    }

    pub fn resolve(env: &ResolverEnv) {
        for item in &env.ast_info.items {
            match item {
                Item::TypeDef(abstract_type_def) => todo!(),
                Item::Struct(abstract_struct) => todo!(),
                Item::Enum(abstract_enum) => todo!(),
                Item::Alias(abstract_alias) => todo!(),
                Item::Var(abstract_var) => todo!(),
                Item::Config(abstract_config) => todo!(),
            }
        }
        todo!("Enving")
    }
}
