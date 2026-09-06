use crate::types::externs::{
    CharacterEncoding, ExternTypeMetadata, ExternTypeRepresentation,
    Signedness, TypeWidth,
};
use chrn_utils::{id_types::InternedId, intern};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum JavaTypeKind {
    Long,
    Int,
    Short,
    Byte,
    Float,
    Double,
    Boolean,
    Char,
    String,
}

impl JavaTypeKind {
    pub const fn metadata(self) -> ExternTypeMetadata {
        match self {
            Self::Long => Self::signed_integer(intern::INTERNED_LONG, 64),
            Self::Int => Self::signed_integer(intern::INTERNED_INT, 32),
            Self::Short => Self::signed_integer(intern::INTERNED_SHORT, 16),
            Self::Byte => Self::signed_integer(intern::INTERNED_BYTE, 8),
            Self::Float => ExternTypeMetadata {
                name_id: InternedId::new(intern::INTERNED_FLOAT_LOWER),
                representation: ExternTypeRepresentation::Float {
                    width: TypeWidth::Fixed(32),
                },
            },
            Self::Double => ExternTypeMetadata {
                name_id: InternedId::new(intern::INTERNED_DOUBLE),
                representation: ExternTypeRepresentation::Float {
                    width: TypeWidth::Fixed(64),
                },
            },
            Self::Boolean => ExternTypeMetadata {
                name_id: InternedId::new(intern::INTERNED_BOOLEAN),
                representation: ExternTypeRepresentation::Bool,
            },
            Self::Char => ExternTypeMetadata {
                name_id: InternedId::new(intern::INTERNED_CHAR),
                representation: ExternTypeRepresentation::Character {
                    encoding: CharacterEncoding::Utf16,
                    width: TypeWidth::Fixed(16),
                },
            },
            Self::String => ExternTypeMetadata {
                name_id: InternedId::new(intern::INTERNED_STRING),
                representation: ExternTypeRepresentation::String {
                    encoding: CharacterEncoding::Utf16,
                },
            },
        }
    }
}

impl JavaTypeKind {
    const fn signed_integer(name_id: u32, width: u16) -> ExternTypeMetadata {
        ExternTypeMetadata {
            name_id: InternedId::new(name_id),
            representation: ExternTypeRepresentation::Integer {
                signedness: Signedness::Signed,
                width: TypeWidth::Fixed(width),
            },
        }
    }
}
