use chrn_utils::{id_types::InternedId, intern};

use crate::script_compiler::helpers::extern_helpers::{ExternKind, ExternNamespace};

//TEST:
pub static JAVA_NAMESPACE: [ExternKind; 1] = [
    ExternKind::Namespace(ExternNamespace::new(
        InternedId::new(intern::INTERNED_JAVA_UPPER),
        &TYPES_NAMESPACE,
    )),
    //
];

pub static TYPES_NAMESPACE: [ExternKind; 1] = [ExternKind::Namespace(ExternNamespace::new(
    // Maybe allow for NOT using the namespace to be an option, but not sure, will just require
    // the java prefix for now just to have the namespaces be distinct.
    InternedId::new(intern::INTERNED_JAVA_LOWER),
    &[ExternKind::Symbol(InternedId::new(intern::INTERNED_INT))],
))];
