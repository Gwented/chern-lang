use chrn_utils::intern;
use lang::types::externs::{ExternPlatformType, rust_types::RustTypeKind};

use crate::script_compiler::{
    extern_helpers::{new_extern_namespace, new_extern_sym},
    helpers::instantiation_symbols::InstantiationSymbolBase,
};

pub static RUST_NAMESPACE_ENTRY: [InstantiationSymbolBase; 1] = [new_extern_namespace(
    intern::INTERNED_RUST_UPPER,
    &RUST_NAMESPACE,
)];

pub static RUST_NAMESPACE: [InstantiationSymbolBase; 1] = [new_extern_namespace(
    intern::INTERNED_TYPES_LOWER,
    &TYPES_NAMESPACE,
)];

pub static TYPES_NAMESPACE: [InstantiationSymbolBase; 1] = [new_extern_namespace(
    intern::INTERNED_RUST_LOWER,
    &RUST_LOWER_NAMESPACE,
)];

/// `rust` which paths to Rust's primitive and standard string types.
pub static RUST_LOWER_NAMESPACE: [InstantiationSymbolBase; 19] = [
    new_extern_sym(
        intern::INTERNED_U8,
        ExternPlatformType::Rust(RustTypeKind::U8),
    ),
    new_extern_sym(
        intern::INTERNED_I8,
        ExternPlatformType::Rust(RustTypeKind::I8),
    ),
    new_extern_sym(
        intern::INTERNED_U16,
        ExternPlatformType::Rust(RustTypeKind::U16),
    ),
    new_extern_sym(
        intern::INTERNED_I16,
        ExternPlatformType::Rust(RustTypeKind::I16),
    ),
    new_extern_sym(
        intern::INTERNED_U32,
        ExternPlatformType::Rust(RustTypeKind::U32),
    ),
    new_extern_sym(
        intern::INTERNED_I32,
        ExternPlatformType::Rust(RustTypeKind::I32),
    ),
    new_extern_sym(
        intern::INTERNED_F32,
        ExternPlatformType::Rust(RustTypeKind::F32),
    ),
    new_extern_sym(
        intern::INTERNED_U64,
        ExternPlatformType::Rust(RustTypeKind::U64),
    ),
    new_extern_sym(
        intern::INTERNED_I64,
        ExternPlatformType::Rust(RustTypeKind::I64),
    ),
    new_extern_sym(
        intern::INTERNED_F64,
        ExternPlatformType::Rust(RustTypeKind::F64),
    ),
    new_extern_sym(
        intern::INTERNED_I128,
        ExternPlatformType::Rust(RustTypeKind::I128),
    ),
    new_extern_sym(
        intern::INTERNED_U128,
        ExternPlatformType::Rust(RustTypeKind::U128),
    ),
    new_extern_sym(
        intern::INTERNED_F128,
        ExternPlatformType::Rust(RustTypeKind::F128),
    ),
    new_extern_sym(
        intern::INTERNED_USIZE,
        ExternPlatformType::Rust(RustTypeKind::Usize),
    ),
    new_extern_sym(
        intern::INTERNED_ISIZE,
        ExternPlatformType::Rust(RustTypeKind::Isize),
    ),
    new_extern_sym(
        intern::INTERNED_BOOL,
        ExternPlatformType::Rust(RustTypeKind::Bool),
    ),
    new_extern_sym(
        intern::INTERNED_CHAR,
        ExternPlatformType::Rust(RustTypeKind::Char),
    ),
    new_extern_sym(
        intern::INTERNED_STR,
        ExternPlatformType::Rust(RustTypeKind::Str),
    ),
    new_extern_sym(
        intern::INTERNED_STRING,
        ExternPlatformType::Rust(RustTypeKind::String),
    ),
];
