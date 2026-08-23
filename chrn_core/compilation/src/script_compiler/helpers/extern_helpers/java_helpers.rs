use chrn_utils::intern;

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

pub static TYPES_NAMESPACE: [InstantiationSymbolBase; 1] = [new_extern_sym(intern::INTERNED_INT)];
