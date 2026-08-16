//! Helper routing structs for config members that are from different sections, which have different
//! semantics
use chrn_utils::{
    id_types::{ImplId, SpannedContainer},
    source_map::source_span::SourceSpan,
};

use crate::parser::ast::ast_exprs::TypeExpr;

/// Struct for routing given a particular config member section origin setting

#[derive(Debug)]
pub(super) struct ConfigMemberContext<'a> {
    /// To root config member
    root_impl_id: ImplId,
    root_span: SourceSpan,
    kind: ConfigMemberContextKind<'a>,
}

#[derive(Debug)]
pub(super) enum ConfigMemberContextKind<'a> {
    Complex(ConfigMemberComplexContext<'a>),
    Override(ConfigMemberOverrideContext),
}

#[derive(Debug)]
pub(super) struct ConfigMemberComplexContext<'a> {
    root_span: SpannedContainer<&'a TypeExpr>,
}

#[derive(Debug)]
pub(super) struct ConfigMemberOverrideContext {}
