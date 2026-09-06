use crate::types::externs::{
    CharacterEncoding, ExternPlatformType, ExternTypeMetadata, ExternTypeRepresentation,
    Signedness, TypeWidth, java_types::JavaTypeKind,
    rust_types::RustTypeKind,
};
use chrn_utils::intern::Intern;

fn assert_metadata(
    interner: &Intern,
    metadata: ExternTypeMetadata,
    expected_name: &str,
    expected_representation: ExternTypeRepresentation,
) {
    assert_eq!(interner.search(metadata.name_id), expected_name);
    assert_eq!(metadata.representation, expected_representation);
}

#[test]
fn java_integer_metadata_matches_language_value_domains() {
    let interner = Intern::init();
    let cases = [
        (JavaTypeKind::Byte, "byte", 8),
        (JavaTypeKind::Short, "short", 16),
        (JavaTypeKind::Int, "int", 32),
        (JavaTypeKind::Long, "long", 64),
    ];

    for (ty, name, width) in cases {
        assert_metadata(
            &interner,
            ExternPlatformType::Java(ty).metadata(),
            name,
            ExternTypeRepresentation::Integer {
                signedness: Signedness::Signed,
                width: TypeWidth::Fixed(width),
            },
        );
    }
}

#[test]
fn java_non_integer_metadata_matches_language_value_domains() {
    let interner = Intern::init();
    let cases = [
        (
            JavaTypeKind::Float,
            "float",
            ExternTypeRepresentation::Float {
                width: TypeWidth::Fixed(32),
            },
        ),
        (
            JavaTypeKind::Double,
            "double",
            ExternTypeRepresentation::Float {
                width: TypeWidth::Fixed(64),
            },
        ),
        (
            JavaTypeKind::Boolean,
            "boolean",
            ExternTypeRepresentation::Bool,
        ),
        (
            JavaTypeKind::Char,
            "char",
            ExternTypeRepresentation::Character {
                encoding: CharacterEncoding::Utf16,
                width: TypeWidth::Fixed(16),
            },
        ),
        (
            JavaTypeKind::String,
            "String",
            ExternTypeRepresentation::String {
                encoding: CharacterEncoding::Utf16,
            },
        ),
    ];

    for (ty, name, representation) in cases {
        assert_metadata(
            &interner,
            ExternPlatformType::Java(ty).metadata(),
            name,
            representation,
        );
    }
}

#[test]
fn rust_numeric_metadata_matches_language_value_domains() {
    let interner = Intern::init();
    let cases = [
        (
            RustTypeKind::U8,
            "u8",
            Signedness::Unsigned,
            TypeWidth::Fixed(8),
        ),
        (
            RustTypeKind::I8,
            "i8",
            Signedness::Signed,
            TypeWidth::Fixed(8),
        ),
        (
            RustTypeKind::U16,
            "u16",
            Signedness::Unsigned,
            TypeWidth::Fixed(16),
        ),
        (
            RustTypeKind::I16,
            "i16",
            Signedness::Signed,
            TypeWidth::Fixed(16),
        ),
        (
            RustTypeKind::U32,
            "u32",
            Signedness::Unsigned,
            TypeWidth::Fixed(32),
        ),
        (
            RustTypeKind::I32,
            "i32",
            Signedness::Signed,
            TypeWidth::Fixed(32),
        ),
        (
            RustTypeKind::U64,
            "u64",
            Signedness::Unsigned,
            TypeWidth::Fixed(64),
        ),
        (
            RustTypeKind::I64,
            "i64",
            Signedness::Signed,
            TypeWidth::Fixed(64),
        ),
        (
            RustTypeKind::U128,
            "u128",
            Signedness::Unsigned,
            TypeWidth::Fixed(128),
        ),
        (
            RustTypeKind::I128,
            "i128",
            Signedness::Signed,
            TypeWidth::Fixed(128),
        ),
        (
            RustTypeKind::Usize,
            "usize",
            Signedness::Unsigned,
            TypeWidth::Pointer,
        ),
        (
            RustTypeKind::Isize,
            "isize",
            Signedness::Signed,
            TypeWidth::Pointer,
        ),
    ];

    for (ty, name, signedness, width) in cases {
        assert_metadata(
            &interner,
            ExternPlatformType::Rust(ty).metadata(),
            name,
            ExternTypeRepresentation::Integer { signedness, width },
        );
    }

    let float_cases = [
        (RustTypeKind::F32, "f32", TypeWidth::Fixed(32)),
        (RustTypeKind::F64, "f64", TypeWidth::Fixed(64)),
        (RustTypeKind::F128, "f128", TypeWidth::Fixed(128)),
    ];

    for (ty, name, width) in float_cases {
        assert_metadata(
            &interner,
            ExternPlatformType::Rust(ty).metadata(),
            name,
            ExternTypeRepresentation::Float { width },
        );
    }
}

#[test]
fn rust_non_numeric_metadata_matches_language_value_domains() {
    let interner = Intern::init();
    let cases = [
        (RustTypeKind::Bool, "bool", ExternTypeRepresentation::Bool),
        (
            RustTypeKind::Char,
            "char",
            ExternTypeRepresentation::Character {
                encoding: CharacterEncoding::UnicodeScalar,
                width: TypeWidth::Fixed(32),
            },
        ),
        (
            RustTypeKind::Str,
            "str",
            ExternTypeRepresentation::String {
                encoding: CharacterEncoding::Utf8,
            },
        ),
        (
            RustTypeKind::String,
            "String",
            ExternTypeRepresentation::String {
                encoding: CharacterEncoding::Utf8,
            },
        ),
    ];

    for (ty, name, representation) in cases {
        assert_metadata(
            &interner,
            ExternPlatformType::Rust(ty).metadata(),
            name,
            representation,
        );
    }
}
