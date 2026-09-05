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
    semantic::{
        hir::hir_impls::LinkedConfigOverrideMemberKind,
        resolution::resolution_concepts::{StaticAccessResult, TypeExprResult},
    },
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
            ConfigRootContextKind::Override(_) => None,
        }
    }

    /// Attempts to get `SymbolId` out of `self`
    pub(super) fn sym_id(&self) -> Option<SymbolId> {
        match self {
            ConfigRootContextKind::Override(ctx) => Some(ctx.override_sym_id),
            ConfigRootContextKind::Complex(_) => None,
        }
    }
}

#[derive(Debug)]
pub(super) struct ConfigRootOverrideContext {
    /// `SymbolId` of the current `override` symbol to go into the namespace of
    pub(super) override_sym_id: SymbolId,
}

impl ConfigRootOverrideContext {
    pub(super) const fn new(override_sym_id: SymbolId) -> Self {
        Self { override_sym_id }
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
            ConfigMemberContextKind::Override(_) => None,
        }
    }

    /// Attempts to get `SymbolId` out of `self`
    pub(super) const fn sym_id(&self) -> Option<SymbolId> {
        match self {
            ConfigMemberContextKind::Override(ctx) => Some(ctx.override_sym_id),
            ConfigMemberContextKind::Complex(_) => None,
        }
    }
}

#[derive(Debug)]
pub(super) struct ConfigMemberComplexContext {
    pub(super) memb_id: MemberId,
}

impl ConfigMemberComplexContext {
    pub(super) fn new(memb_id: MemberId) -> Self {
        Self { memb_id }
    }
}

#[derive(Debug)]
pub(super) struct ConfigMemberOverrideContext {
    /// `SymbolId` of the current `override` symbol to go into the namespace of
    pub override_sym_id: SymbolId,
    /// Is `Option` because if it's a global override usage like "override C {}" not being linked
    /// would just be another state.
    pub linked_kind: LinkedConfigOverrideMemberKind,
}

impl ConfigMemberOverrideContext {
    pub(super) const fn new(
        override_sym_id: SymbolId,
        linked_kind: LinkedConfigOverrideMemberKind,
    ) -> Self {
        Self {
            override_sym_id,
            linked_kind,
        }
    }
}
