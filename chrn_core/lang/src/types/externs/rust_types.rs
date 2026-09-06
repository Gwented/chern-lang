use crate::types::externs::{
    CharacterEncoding, ExternTypeMetadata, ExternTypeRepresentation, Signedness, TypeWidth,
};
use chrn_utils::{id_types::InternedId, intern};

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum RustTypeKind {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    F32,
    U64,
    I64,
    F64,
    I128,
    U128,
    F128,
    Usize,
    Isize,
    Bool,
    Char,
    Str,
    String,
}

impl RustTypeKind {
    pub const fn metadata(self) -> ExternTypeMetadata {
        let representation = match self {
            RustTypeKind::U8
            | RustTypeKind::U16
            | RustTypeKind::U32
            | RustTypeKind::U64
            | RustTypeKind::U128
            | RustTypeKind::Usize => ExternTypeRepresentation::Integer {
                signedness: Signedness::Unsigned,
                width: self.width(),
            },
            RustTypeKind::I8
            | RustTypeKind::I16
            | RustTypeKind::I32
            | RustTypeKind::I64
            | RustTypeKind::I128
            | RustTypeKind::Isize => ExternTypeRepresentation::Integer {
                signedness: Signedness::Signed,
                width: self.width(),
            },
            RustTypeKind::F32 | RustTypeKind::F64 | RustTypeKind::F128 => {
                ExternTypeRepresentation::Float {
                    width: self.width(),
                }
            }
            RustTypeKind::Bool => ExternTypeRepresentation::Bool,
            RustTypeKind::Char => ExternTypeRepresentation::Character {
                encoding: CharacterEncoding::UnicodeScalar,
                width: TypeWidth::Fixed(32),
            },
            RustTypeKind::Str | RustTypeKind::String => ExternTypeRepresentation::String {
                encoding: CharacterEncoding::Utf8,
            },
        };

        ExternTypeMetadata {
            name_id: InternedId::new(self.name_id()),
            representation,
        }
    }
}

impl RustTypeKind {
    const fn name_id(self) -> u32 {
        match self {
            Self::U8 => intern::INTERNED_U8,
            Self::I8 => intern::INTERNED_I8,
            Self::U16 => intern::INTERNED_U16,
            Self::I16 => intern::INTERNED_I16,
            Self::U32 => intern::INTERNED_U32,
            Self::I32 => intern::INTERNED_I32,
            Self::F32 => intern::INTERNED_F32,
            Self::U64 => intern::INTERNED_U64,
            Self::I64 => intern::INTERNED_I64,
            Self::F64 => intern::INTERNED_F64,
            Self::I128 => intern::INTERNED_I128,
            Self::U128 => intern::INTERNED_U128,
            Self::F128 => intern::INTERNED_F128,
            Self::Usize => intern::INTERNED_USIZE,
            Self::Isize => intern::INTERNED_ISIZE,
            Self::Bool => intern::INTERNED_BOOL,
            Self::Char => intern::INTERNED_CHAR,
            Self::Str => intern::INTERNED_STR,
            Self::String => intern::INTERNED_STRING,
        }
    }

    const fn width(self) -> TypeWidth {
        match self {
            Self::U8 | Self::I8 => TypeWidth::Fixed(8),
            Self::U16 | Self::I16 => TypeWidth::Fixed(16),
            Self::U32 | Self::I32 | Self::F32 => TypeWidth::Fixed(32),
            Self::U64 | Self::I64 | Self::F64 => TypeWidth::Fixed(64),
            Self::U128 | Self::I128 | Self::F128 => TypeWidth::Fixed(128),
            Self::Usize | Self::Isize => TypeWidth::Pointer,
            Self::Bool | Self::Char | Self::Str | Self::String => {
                panic!("only numeric Rust types have a numeric width")
            }
        }
    }
}
