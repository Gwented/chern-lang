//! Helper routing structs for config members that are from different sections, which have different
//! semantics
use chrn_utils::id_types::{SymbolId, TypeId};

use crate::parser::ast::ast_exprs::TypeExpr;

/// Struct for routing given a particular config member section origin setting

// #[derive(Debug)]
// pub(super) struct ConfigMemberContext<'a> {
//     /// To root config member
//     kind: ConfigMemberContextKind<'a>,
// }
//
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
