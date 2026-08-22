use chrn_utils::intern;

use crate::script_compiler::{
    extern_helpers::{new_extern_namespace, new_extern_sym},
    helpers::common_helpers::CommonSymbolBase,
};

// pub static JAVA_NAMESPACE_ENTRY: [CommonKind; 1] = [CommonKind::Namespace(new_extern_namespace(
//     intern::INTERNED_JAVA_UPPER,
//     &JAVA_NAMESPACE,
// ))];

pub static JAVA_NAMESPACE_ENTRY: [CommonSymbolBase; 1] = [new_extern_namespace(
    intern::INTERNED_JAVA_UPPER,
    &JAVA_NAMESPACE,
)];

// pub static JAVA_NAMESPACE: [CommonKind; 1] = [
//     CommonKind::Namespace(new_extern_namespace(
//         intern::INTERNED_TYPES_LOWER,
//         &TYPES_NAMESPACE,
//     )),
//     //
// ];

pub static JAVA_NAMESPACE: [CommonSymbolBase; 1] = [new_extern_namespace(
    intern::INTERNED_TYPES_LOWER,
    &TYPES_NAMESPACE,
)];

// pub static TYPES_NAMESPACE: [CommonKind; 1] = [CommonKind::Namespace(new_extern_namespace(
//     // Maybe allow for NOT using the namespace to be an option, but not sure, will just require
//     // the java prefix for now just to have the namespaces be distinct.
//     intern::INTERNED_JAVA_LOWER,
//     &[CommonKind::Symbol(new_extern_sym(intern::INTERNED_INT))],
// ))];

pub static TYPES_NAMESPACE: [CommonSymbolBase; 1] = [new_extern_sym(intern::INTERNED_INT)];
