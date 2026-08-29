use chrn_utils::{
    id_types::TypeId, intern::Intern, source_map::source_span::SourceSpan,
    utils::containers::SpannedContainer,
};

use crate::{
    lookup::scopes::scopes_concepts::{
        self, AssociatedScopeKind, ScopeLookupPattern, ScopeLookupPreferenceFlags, ScopeType,
        SymbolLookupOutput,
    },
    parser::ast::{
        ast_exprs::{AbstractGeneric, PathSegment, TypeExpr},
        ast_stmts::AbstractOptionAssignment,
    },
    resolvers::resolver_env::ResolverEnv,
    script_compiler::ScriptCompiler,
    semantic::{
        preset_reporter::{self, preset_err::PresetErr},
        resolution::{self, StaticAccessOption, StaticAccessResult, TypeExprResult},
    },
};

// Incredible name
// With is a little odd since, it's not with it's just returning, but ret_preset seems...completely
// fine actually
/// Convenience over `resolution::resolve_type_expr` which covers preset handling boiler-plate
pub(crate) fn resolve_type_expr_ret_preset(
    compiler: &mut ScriptCompiler,
    associated_scope: AssociatedScopeKind,
    sp_ty_expr: &SpannedContainer<TypeExpr>,
    scope_type: ScopeType,
    lookup_pattern: ScopeLookupPattern,
    interner: &Intern,
    env: &ResolverEnv,
) -> Result<TypeId, PresetErr> {
    match resolution::resolve_type_expr(
        compiler,
        associated_scope,
        sp_ty_expr,
        scope_type,
        lookup_pattern,
        env,
    ) {
        TypeExprResult::Type(type_id) => Ok(type_id),
        res => {
            let preset_err =
                preset_reporter::type_expr_result_to_preset_err(compiler, interner, &res, env)
                    .expect("Confirmed by match");
            Err(preset_err)
        }
    }
}

/// Convenience over `resolution::resolve_static_access` which covers preset handling boiler-plate
pub(crate) fn resolve_static_access_ret_preset(
    compiler: &ScriptCompiler,
    sp_path_segs: &[SpannedContainer<PathSegment>],
    current_scope: AssociatedScopeKind,
    scope_type: ScopeType,
    lookup_pref: ScopeLookupPreferenceFlags,
    opt: StaticAccessOption,
    interner: &Intern,
    env: &ResolverEnv,
) -> Result<AssociatedScopeKind, PresetErr> {
    match resolution::resolve_static_access(
        compiler,
        sp_path_segs,
        current_scope,
        scope_type,
        lookup_pref,
        opt,
    ) {
        StaticAccessResult::Scope(scope) => Ok(scope),
        res => {
            let preset_err =
                preset_reporter::static_access_result_to_preset_err(interner, &res, env)
                    .expect("Confirmed by match");
            Err(preset_err)
        }
    }
}

/// Convenience over `resolution::resolve_generic` which covers preset handling boiler-plate
pub(crate) fn resolve_generic_ret_preset(
    compiler: &mut ScriptCompiler,
    // Span me a new container
    generic: &AbstractGeneric,
    associated_scope: AssociatedScopeKind,
    ty_expr_span: SourceSpan,
    scope_type: ScopeType,
    interner: &Intern,
    env: &ResolverEnv,
) -> Result<TypeId, PresetErr> {
    match resolution::resolve_generic(
        compiler,
        generic,
        associated_scope,
        ty_expr_span,
        scope_type,
        env,
    ) {
        TypeExprResult::Type(type_id) => Ok(type_id),
        res => {
            let preset_err =
                preset_reporter::type_expr_result_to_preset_err(compiler, interner, &res, env)
                    .expect("Confirmed by match");
            Err(preset_err)
        }
    }
}

// !@
pub(crate) fn resolve_sym_id_ret_preset() {
    todo!()
}
