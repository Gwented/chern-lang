use crate::types::externs::{java_types::JavaTypeKind, rust_types::RustTypeKind};
use chrn_utils::{id_types::InternedId, intern};
//TEST: None of this is finalityily finallliality final. Question mark.

pub mod java_types;
pub mod rust_types;

/// Exact type supplied by a supported target language.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ExternPlatformType {
    Java(JavaTypeKind),
    Rust(RustTypeKind),
}

impl ExternPlatformType {
    // Should we rely on "__java" or something special for this or no?
    // Who are you talking to
    /// Identifier of the platform of the current type.
    /// Returns in lower case, like "java", "rust", etsy.
    pub fn platform_name(self) -> InternedId {
        let id = match self {
            ExternPlatformType::Java(_) => intern::INTERNED_JAVA_LOWER,
            ExternPlatformType::Rust(_) => intern::INTERNED_RUST_LOWER,
        };
        InternedId::new(id)
    }
}

/// Metadata needed to compare a chrn type with an external mapping target.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ExternTypeMetadata {
    /// Language type identifier
    pub name_id: InternedId,
    /// Conceptual values representable by the target type.
    pub representation: ExternTypeRepresentation,
}

/// Value representations currently understood by chrn's external type mappings.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ExternTypeRepresentation {
    Integer {
        signedness: Signedness,
        width: TypeWidth,
    },
    Float {
        width: TypeWidth,
    },
    Bool,
    Character {
        encoding: CharacterEncoding,
        width: TypeWidth,
    },
    String {
        encoding: CharacterEncoding,
    },
    ArbitraryInteger {
        signedness: Signedness,
    },
    ArbitraryFloat,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Signedness {
    Signed,
    Unsigned,
}

/// Storage width when it is part of a type's value representation.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TypeWidth {
    Fixed(u16),
    Pointer,
    Runtime,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CharacterEncoding {
    Ascii,
    Utf8,
    Utf16,
    Utf32,
    UnicodeScalar,
    Platform,
}

impl ExternPlatformType {
    pub const fn metadata(self) -> ExternTypeMetadata {
        match self {
            Self::Java(ty) => ty.metadata(),
            Self::Rust(ty) => ty.metadata(),
        }
    }
}
