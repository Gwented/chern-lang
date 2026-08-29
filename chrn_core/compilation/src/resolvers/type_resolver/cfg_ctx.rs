//! Helper routing structs for config members that are from different sections, which have different
//! semantics
use chrn_utils::{
    id_types::{MemberId, SymbolId, TypeId},
    utils::containers::SpannedContainer,
};

use crate::{
    lookup::member_lookup::MemberLookupResult,
    parser::ast::ast_exprs::PathSegment,
    semantic::resolution::resolution_concepts::{StaticAccessResult, TypeExprResult},
};

/// Struct for routing given a particular config member section origin setting

// #[derive(Debug)]
// pub(super) struct ConfigMemberContext<'a> {
//     /// To root config member
//     kind: ConfigMemberContextKind<'a>,
// }
//

#[derive(Debug)]
pub(super) enum ConfigMemberResult {
    Member(MemberLookupResult),
    Namespace(StaticAccessResult),
}

pub(super) enum ConfigMemberOutput {
    Member(MemberId),
    Namespace(SymbolId),
}

#[derive(Debug)]
pub(super) enum ConfigMemberError {}

//TODO: We may need 3 different contexts embedded total.
//1: Complex, which only takes in a type
//2: Override namespace from an intrinsic like "JAVA"
//3: Override struct, which through the namespace alters it's particular internals.

#[derive(Debug)]
pub(super) enum ConfigMemberContextKind<'a> {
    // Config member member context
    Complex(ConfigMemberComplexContext),
    Override(ConfigMemberOverrideContext<'a>),
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
pub(super) struct ConfigMemberOverrideContext<'a> {
    pub(super) sym_id: SymbolId,
    pub(super) sp_path_segs: &'a [SpannedContainer<PathSegment>],
}

impl<'a> ConfigMemberOverrideContext<'a> {
    pub(super) fn new(sym_id: SymbolId, sp_path_segs: &'a [SpannedContainer<PathSegment>]) -> Self {
        Self {
            sym_id,
            sp_path_segs,
        }
    }
}
