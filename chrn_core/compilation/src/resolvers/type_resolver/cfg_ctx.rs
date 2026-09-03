//! Helper routing structs for config members that are from different sections, which have different
//! semantics
use chrn_utils::{
    id_types::{MemberId, SymbolId, TypeId},
    utils::containers::SpannedContainer,
};

use crate::{
    lookup::member_lookup::MemberLookupResult,
    parser::ast::ast_exprs::PathSegment,
    script_compiler::ScriptCompiler,
    semantic::resolution::resolution_concepts::{StaticAccessResult, TypeExprResult},
};

/// Struct for routing given a particular config member section origin setting

//TODO: We may need 3 different contexts embedded total.
//1: Complex, which only takes in a type
//2: Override namespace from an intrinsic like "JAVA"
//3: Override struct, which through the namespace alters it's particular internals.

/// This context represents the data associated with the config root, which varies by how the root
/// was defined, which embeds different semantics.
///
// Say more, the class would love to hear more.
// Um.
/// This needs to exist along with the config member ctx due to the root and member needing to work
/// with each other.
#[derive(Debug)]
pub(super) enum ConfigRootContextKind {
    Override(ConfigRootOverrideContext),
    Complex(ConfigRootComplexContext),
}

impl ConfigRootContextKind {
    /// Attempts to get `TypeId` out of `self`
    pub(super) const fn type_id(&self) -> Option<TypeId> {
        match self {
            ConfigRootContextKind::Complex(ctx) => Some(ctx.type_id),
            ConfigRootContextKind::Override(ctx) => None,
        }
    }

    /// Attempts to get `SymbolId` out of `self`
    pub(super) fn sym_id(&self) -> Option<SymbolId> {
        match self {
            ConfigRootContextKind::Override(ctx) => Some(ctx.sym_id),
            ConfigRootContextKind::Complex(_) => None,
        }
    }
}

#[derive(Debug)]
pub(super) struct ConfigRootOverrideContext {
    pub(super) sym_id: SymbolId,
}

impl ConfigRootOverrideContext {
    pub(super) const fn new(sym_id: SymbolId) -> Self {
        Self { sym_id }
    }
}

#[derive(Debug)]
pub(super) struct ConfigRootComplexContext {
    pub(super) type_id: TypeId,
}

impl ConfigRootComplexContext {
    pub(super) const fn new(type_id: TypeId) -> Self {
        Self { type_id }
    }
}

#[derive(Debug)]
pub(super) enum ConfigMemberContextKind {
    // Config member member context
    // Maybe rename to base? The section discernment no longer exists so maybe remove the "complex"
    // naming since they are both inside complex to begin with?
    Complex(ConfigMemberComplexContext),
    Override(ConfigMemberOverrideContext),
}

impl ConfigMemberContextKind {
    /// Attempts to get `MemberId` out of `self`
    pub(super) const fn memb_id(&self) -> Option<MemberId> {
        match self {
            ConfigMemberContextKind::Complex(ctx) => Some(ctx.memb_id),
            ConfigMemberContextKind::Override(ctx) => None,
        }
    }
}

#[derive(Debug)]
pub(super) struct ConfigMemberComplexContext {
    /// `MemberId` of `self`
    pub(super) memb_id: MemberId,
}

impl ConfigMemberComplexContext {
    pub(super) fn new(memb_id: MemberId) -> Self {
        Self { memb_id }
    }
}

#[derive(Debug)]
pub(super) struct ConfigMemberOverrideContext {}

impl<'a> ConfigMemberOverrideContext {
    pub(super) const fn new() -> Self {
        Self {}
    }
}
