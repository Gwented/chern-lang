use std::fmt::Display;

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
    //TODO: Change to O(1) with interned id -> idx mapppppppppppppppppppppppppppppping
    //Huh
    //We would need a static hashmap to do this since the name is the only actual entity
    //that can be checkeddddjdoiajdod

    /// Attempts to find option from the given identifier and returns it's `OptionSchema`
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

/// Represents a configs options, that are preloaded by the compiler as schemas to follow
#[derive(Debug)]
pub struct OptionSchema {
    pub name_id: InternedId,
    pub boundaries: Option<OptionSchemaConstraint>,
}

impl OptionSchema {
    pub const fn new(
        name_id: InternedId,
        boundaries: Option<OptionSchemaConstraint>,
    ) -> OptionSchema {
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
    Member,
}

impl Display for ConfigSchemaKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let out = match self {
            ConfigSchemaKind::Struct => "struct",
            ConfigSchemaKind::Enum => "enum",
            ConfigSchemaKind::Member => "member",
        };
        write!(f, "{out}")
    }
}

/// All known preset schemas and the options associated with them
pub static PRESET_CONFIG_SCHEMAS: [ConfigSchema; 3] = [
    // ONLY 3 VALID SCHEMAS RIGHT NOW
    ConfigSchema::new(ConfigSchemaKind::Struct, &[OPTION_CASES, OPTION_IDENTS]),
    ConfigSchema::new(ConfigSchemaKind::Enum, &[OPTION_CASES, OPTION_IDENTS]),
    ConfigSchema::new(
        ConfigSchemaKind::Member,
        &[OPTION_CASES, OPTION_IDENTS, OPTION_DEFAULT_VAL],
    ),
];

#[derive(Debug, Clone)]
pub enum OptionSchemaConstraint {
    Boundaries(TypeBoundaryFlags),
    SameTypeAsConfig,
    // None,
}

const OPTION_DEFAULT_VAL: OptionSchema = OptionSchema::new(
    InternedId::new(intern::INTERNED_DEFAULT_VAL),
    Some(OptionSchemaConstraint::SameTypeAsConfig),
);
const OPTION_IDENTS: OptionSchema = OptionSchema::new(
    InternedId::new(intern::INTERNED_IDENTS),
    Some(OptionSchemaConstraint::Boundaries(TypeBoundaryFlags::STR)),
);
const OPTION_CASES: OptionSchema = OptionSchema::new(
    InternedId::new(intern::INTERNED_CASES),
    Some(OptionSchemaConstraint::Boundaries(TypeBoundaryFlags::STR)),
);

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
        ConfigSchemaKind::Member => &PRESET_CONFIG_SCHEMAS[2],
    }
}
