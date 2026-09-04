use chrn_utils::{
    err_codes::ErrorCode,
    id_types::InternedId,
    intern::Intern,
    source_map::source_diagnostic::{
        DiagnosticLevel, SourceDiagnostic, annotations::AnnotationKind,
    },
    utils::containers::SpannedContainer,
};

use crate::{
    lookup::scopes::{
        self,
        scopes_concepts::{
            AssociatedScopeKind, ScopeLookupPattern, ScopeLookupPreferenceFlags, ScopeType,
            SymbolLookupOutput,
        },
    },
    parser::ast::ast_exprs::PathSegment,
    resolvers::resolver_env::ResolverEnv,
    script_compiler::ScriptCompiler,
    semantic::{
        preset_reporter::preset_err::{LookupError, PresetErr},
        resolution::{resolution_concepts::StaticAccessOption, resolution_helpers},
    },
};

// Ok. I see.
/// Looks up ambiguous expr and expects a `SymbolLookupOutput`.
/// Generates an error upon seeing a type.
pub fn find_sym_id_with_static_ret_preset(
    compiler: &ScriptCompiler,
    initial_scope: AssociatedScopeKind,
    sp_path_segs: &[SpannedContainer<PathSegment>],
    scope_type: ScopeType,
    opt: StaticAccessOption,
    lookup_pat: ScopeLookupPattern,
    lookup_pref: ScopeLookupPreferenceFlags,
    interner: &Intern,
    env: &ResolverEnv,
) -> Result<SymbolLookupOutput, PresetErr> {
    let last_scope = resolution_helpers::resolve_static_access_ret_preset(
        compiler,
        sp_path_segs,
        initial_scope,
        scope_type,
        lookup_pref,
        opt,
        interner,
        env,
    )?;

    let last_seg = &sp_path_segs[sp_path_segs.len() - 1];

    match &last_seg.inner {
        PathSegment::Ident(interned_id) => {
            //TODO: preset err for this maybe
            // Second request for a ret preset version
            find_sym_id_ret_preset(
                compiler,
                last_scope,
                SpannedContainer::new(*interned_id, last_seg.span),
                scope_type,
                lookup_pat,
                lookup_pref,
            )
        }
        // This seems wrong
        PathSegment::Generic(_) => {
            let builder = SourceDiagnostic::builder(
                ErrorCode::GenericsErr.into(),
                DiagnosticLevel::Error,
                "Not expecting a type",
                env.region.path_id,
            )
            .add_annotation(last_seg.span, AnnotationKind::Primary, None);
            return Err(PresetErr::General(builder));
        }
    }
}

/// Convenience over `scopes::find_sym_id` which covers preset handling boiler-plate
pub fn find_sym_id_ret_preset(
    compiler: &ScriptCompiler,
    associated_scope: AssociatedScopeKind,
    sp_target_name_id: SpannedContainer<InternedId>,
    scope_type: ScopeType,
    lookup_pat: ScopeLookupPattern,
    lookup_pref: ScopeLookupPreferenceFlags,
) -> Result<SymbolLookupOutput, PresetErr> {
    match scopes::find_sym_id(
        compiler,
        associated_scope,
        sp_target_name_id.inner,
        scope_type,
        lookup_pat,
        lookup_pref,
    ) {
        Some(out) => Ok(out),
        None => {
            let lookup_err = LookupError::SymbolNotFound {
                sp_invalid_name_id: sp_target_name_id,
                scope_searched: associated_scope,
            };
            Err(lookup_err.into())
        }
    }
}

/// Convenience over `scopes::find_sym_id` which covers preset handling boiler-plate
pub fn find_sym_id_intrinsic_ret_preset(
    compiler: &ScriptCompiler,
    associated_scope: AssociatedScopeKind,
    sp_target_name_id: SpannedContainer<InternedId>,
    scope_type: ScopeType,
) -> Result<SymbolLookupOutput, PresetErr> {
    match scopes::find_sym_id_intrinsic(
        compiler,
        associated_scope,
        sp_target_name_id.inner,
        scope_type,
    ) {
        Some(out) => Ok(out),
        None => {
            let lookup_err = LookupError::SymbolNotFound {
                sp_invalid_name_id: sp_target_name_id,
                scope_searched: associated_scope,
            };
            Err(lookup_err.into())
        }
    }
}
