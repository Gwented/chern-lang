use chrn_utils::{id_types::InternedId, intern};

use crate::types::boundaries::TypeBoundaryFlags;

/// Represent a configurataion description that must be followed,
#[derive(Debug)]
pub struct ConfigSchema {
    pub kind: ConfigSchemaKind,
    pub opt_schema: &'static [OptionSchema],
}

impl ConfigSchema {
    pub const fn new(kind: ConfigSchemaKind, opt_schema: &'static [OptionSchema]) -> ConfigSchema {
        ConfigSchema { kind, opt_schema }
    }

    /// Attempts to find option from the given identifier, then return it's `OptionSchema`
    pub fn get_opt(&self, target_name_id: InternedId) -> Option<&OptionSchema> {
        self.opt_schema
            .iter()
            .find(|opt| opt.name_id == target_name_id)
    }

    /// Attempts to find option from the given identifier
    ///
    /// Returns `true` if present, `false` if not
    pub fn has_opt(&self, target_name_id: InternedId) -> bool {
        self.opt_schema
            .iter()
            .any(|opt| opt.name_id == target_name_id)
    }
}

/// Represents a configurations options, that are preloaded by the compiler as schemas to follow
#[derive(Debug)]
pub struct OptionSchema {
    pub name_id: InternedId,
    pub boundaries: Option<TypeBoundaryFlags>,
}

impl OptionSchema {
    pub const fn new(name_id: InternedId, boundaries: Option<TypeBoundaryFlags>) -> OptionSchema {
        OptionSchema {
            name_id,
            boundaries,
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
pub static PRESET_CONFIG_SCHEMAS: [ConfigSchema; 3] = [
    // ONLY 3 VALID SCHEMAS RIGHT NOW
    ConfigSchema::new(ConfigSchemaKind::Struct, &[OPTION_CASES, OPTION_IDENTS]),
    ConfigSchema::new(ConfigSchemaKind::Enum, &[OPTION_CASES, OPTION_IDENTS]),
    ConfigSchema::new(
        ConfigSchemaKind::Field,
        &[OPTION_CASES, OPTION_IDENTS, OPTION_DEFAULT_VAL],
    ),
];

const OPTION_DEFAULT_VAL: OptionSchema =
    OptionSchema::new(InternedId::new(intern::INTERNED_DEFAULT_VAL), None);
const OPTION_IDENTS: OptionSchema =
    OptionSchema::new(InternedId::new(intern::INTERNED_IDENTS), None);
const OPTION_CASES: OptionSchema = OptionSchema::new(InternedId::new(intern::INTERNED_CASES), None);

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
        ConfigSchemaKind::Struct => &PRESET_CONFIG_SCHEMAS[0],
        ConfigSchemaKind::Enum => &PRESET_CONFIG_SCHEMAS[1],
        ConfigSchemaKind::Field => &PRESET_CONFIG_SCHEMAS[2],
    }
}
