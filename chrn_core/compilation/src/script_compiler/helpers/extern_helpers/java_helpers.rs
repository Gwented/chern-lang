use chrn_utils::intern;
use lang::types::externs::{ExternPlatformType, java_types::JavaTypeKind};

use crate::script_compiler::{
    extern_helpers::{new_extern_namespace, new_extern_sym},
    helpers::instantiation_symbols::InstantiationSymbolBase,
};

pub static JAVA_NAMESPACE_ENTRY: [InstantiationSymbolBase; 1] = [new_extern_namespace(
    intern::INTERNED_JAVA_UPPER,
    &JAVA_NAMESPACE,
)];

pub static JAVA_NAMESPACE: [InstantiationSymbolBase; 1] = [new_extern_namespace(
    intern::INTERNED_TYPES_LOWER,
    &TYPES_NAMESPACE,
)];

pub static TYPES_NAMESPACE: [InstantiationSymbolBase; 1] = [new_extern_namespace(
    intern::INTERNED_JAVA_LOWER,
    &JAVA_LOWER_NAMESPACE,
)];

/// `java` which paths to Java's primitive and standard string types.
pub static JAVA_LOWER_NAMESPACE: [InstantiationSymbolBase; 9] = [
    new_extern_sym(
        intern::INTERNED_LONG,
        ExternPlatformType::Java(JavaTypeKind::Long),
    ),
    new_extern_sym(
        intern::INTERNED_INT,
        ExternPlatformType::Java(JavaTypeKind::Int),
    ),
    new_extern_sym(
        intern::INTERNED_SHORT,
        ExternPlatformType::Java(JavaTypeKind::Short),
    ),
    new_extern_sym(
        intern::INTERNED_BYTE,
        ExternPlatformType::Java(JavaTypeKind::Byte),
    ),
    new_extern_sym(
        intern::INTERNED_FLOAT_LOWER,
        ExternPlatformType::Java(JavaTypeKind::Float),
    ),
    new_extern_sym(
        intern::INTERNED_DOUBLE,
        ExternPlatformType::Java(JavaTypeKind::Double),
    ),
    new_extern_sym(
        intern::INTERNED_BOOLEAN,
        ExternPlatformType::Java(JavaTypeKind::Boolean),
    ),
    new_extern_sym(
        intern::INTERNED_CHAR,
        ExternPlatformType::Java(JavaTypeKind::Char),
    ),
    new_extern_sym(
        intern::INTERNED_STRING,
        ExternPlatformType::Java(JavaTypeKind::String),
    ),
];
