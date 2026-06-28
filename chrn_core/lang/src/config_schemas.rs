use chrn_utils::{id_types::InternedId, intern};

use crate::types::type_constraints::TypeConstraintFlags;

/// Represent a configurataion description that must be followed,
#[derive(Debug)]
pub struct ConfigSchema {
    pub kind: ConfigSchemaKind,
    pub option_schema: &'static [OptionSchema],
}

impl ConfigSchema {
    pub const fn new(
        kind: ConfigSchemaKind,
        option_schema: &'static [OptionSchema],
    ) -> ConfigSchema {
        ConfigSchema {
            kind,
            option_schema,
        }
    }
}

/// Represents a configurations options, that are preloaded by the compiler as schemas to follow
#[derive(Debug)]
pub struct OptionSchema {
    name_id: InternedId,
    constraints: Option<TypeConstraintFlags>,
}

impl OptionSchema {
    pub const fn new(
        name_id: InternedId,
        constraints: Option<TypeConstraintFlags>,
    ) -> OptionSchema {
        OptionSchema {
            name_id,
            constraints,
        }
    }
}

#[derive(Debug)]
pub enum ConfigSchemaKind {
    Struct,
    Enum,
    Field,
}

/// All known preset schemas and the kinds associated with it
pub static PRESET_CONFIG_SCHEMAS: [ConfigSchema; 1] = [ConfigSchema::new(
    ConfigSchemaKind::Field,
    &[DEFAULT_VAL_OPTION],
)];

const DEFAULT_VAL_OPTION: OptionSchema =
    OptionSchema::new(InternedId::new(intern::INTERNED_DEFAULT_VALUE), None);

// pub const fn load_schemas() -> &'static [ConfigSchema; 1] {
// Default value
// let default_val_opt: OptionSchema =
//     OptionSchema::new(InternedId::new(intern::INTERNED_DEFAULT_VALUE), None);
//
// // Field Schema
// let field_schema = ConfigSchema {
//     kind: ConfigSchemaKind::Field,
//     option_schema: &'static [default_val_opt],
// };
//
// &[field_schema]
// }

pub const fn get_cfg_schema(kind: ConfigSchemaKind) -> &'static ConfigSchema {
    match kind {
        ConfigSchemaKind::Field => &PRESET_CONFIG_SCHEMAS[0],
        ConfigSchemaKind::Struct => todo!(),
        ConfigSchemaKind::Enum => todo!(),
    }
}
