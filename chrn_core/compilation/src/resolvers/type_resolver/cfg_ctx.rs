//! Helper routing structs for config members that are from different sections, which have different
//! semantics
use chrn_utils::id_types::{SymbolId, TypeId};

/// Struct for routing given a particular config member section origin setting

// #[derive(Debug)]
// pub(super) struct ConfigMemberContext<'a> {
//     /// To root config member
//     kind: ConfigMemberContextKind<'a>,
// }
//

#[derive(Debug)]
pub(super) enum ConfigMemberOutput {
    Namespace(SymbolId),
    Type(TypeId),
}

//TODO: We may need 3 different contexts embedded total.
//1: Complex, which only takes in a type
//2: Override namespace from an intrinsic like "JAVA"
//3: Override struct, which through the namespace alters it's particular internals.

#[derive(Debug)]
pub(super) enum ConfigMemberContextKind {
    Complex(ConfigMemberComplexContext),
    Override(ConfigMemberOverrideContext),
}

#[derive(Debug)]
pub(super) struct ConfigMemberComplexContext {
    pub(super) type_id: TypeId,
}

impl ConfigMemberComplexContext {
    pub(super) fn new(type_id: TypeId) -> Self {
        Self { type_id }
    }
}

#[derive(Debug)]
pub(super) struct ConfigMemberOverrideContext {
    pub(super) sym_id: SymbolId,
}

impl ConfigMemberOverrideContext {
    pub(super) fn new(sym_id: SymbolId) -> Self {
        Self { sym_id }
    }
}
