use chrn_utils::{
    id_types::{InternedId, SymbolId, TypeId},
    loop_abort,
};
use lang::{
    config_schemas::{self, ConfigSchema, ConfigSchemaKind},
    values::Value,
};

use crate::{script_compiler::ScriptCompiler, semantic::hir::hir_concepts::Type};

/// Result type for schema lookups. This exists due to the fact that there is no `Ok` or `Err`
/// inherit concept behind whether or not something was found.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum SchemaResult {
    /// The schema was...valid
    Valid,
    UnknownSchemaName,
}

//TEST: This language construct is still confusing to find the best version of.
/// Uses `TypeId` to search for if there is a schema available to match contents against
pub fn get_schema_from_type_id(
    compiler: &ScriptCompiler,
    mut current_type_id: TypeId,
) -> Option<&'static ConfigSchema> {
    for _ in 0..chrn_utils::MAX_LOOPS {
        match &compiler.types[current_type_id].ty {
            Type::Struct(_) => {
                return Some(config_schemas::get_cfg_schema(ConfigSchemaKind::Struct));
            }
            Type::Enum(_) => {
                return Some(config_schemas::get_cfg_schema(ConfigSchemaKind::Enum));
            }
            Type::Deferred(type_id) => current_type_id = *type_id,
            Type::Alias(_)
            | Type::Boundaries(_)
            | Type::BuiltinType(_)
            | Type::TypeDef(_)
            | Type::Func(_)
            | Type::Unknown => return None,
        }
    }

    loop_abort!();
}

// Will move
pub fn validate_opt(
    compiler: &ScriptCompiler,
    schema: &ConfigSchema,
    opt_name_id: InternedId,
    opt_values: &[Value],
) -> SchemaResult {
    match schema.kind {
        ConfigSchemaKind::Struct => todo!(),
        ConfigSchemaKind::Enum => {
            if let Some(schema_opt) = schema.get_opt(opt_name_id) {
                match schema_opt.boundaries {
                    Some(boundaries) => {
                        for val in opt_values {
                            match val {
                                Value::I64(_) => todo!(),
                                Value::F64(_) => todo!(),
                                Value::Bool(_) => todo!(),
                                Value::Char(_) => todo!(),
                                Value::Func(_) => todo!(),
                                Value::Tuple(_) => todo!(),
                                Value::Array(_) => todo!(),
                                Value::InternedStr(interned_id) => todo!(),
                                Value::RuntimeStr(_) => todo!(),
                                Value::Unknown => todo!(),
                            }
                        }
                        todo!()
                    }
                    // Nothing to actually check against value-wise since there are no boundaries
                    None => return SchemaResult::Valid,
                }
            } else {
                // get_opt failed
                SchemaResult::UnknownSchemaName
            }
        }
        ConfigSchemaKind::Field => todo!(),
    }
}

pub fn has_identifier() {}
