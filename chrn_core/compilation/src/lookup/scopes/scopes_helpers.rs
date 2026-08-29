use chrn_utils::{id_types::InternedId, utils::containers::SpannedContainer};

use crate::{
    lookup::scopes::{
        self,
        scopes_concepts::{
            AssociatedScopeKind, ScopeLookupPattern, ScopeLookupPreferenceFlags, ScopeType,
            SymbolLookupOutput,
        },
    },
    script_compiler::ScriptCompiler,
    semantic::preset_reporter::preset_err::{LookupError, PresetErr},
};

/// Convenience over `scopes::find_sym_id` which covers preset handling boiler-plate
pub fn find_sym_id(
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
