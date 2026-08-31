//! Scope traversal functions
pub mod scopes_concepts;
pub mod scopes_helpers;

use chrn_utils::{
    id_types::{InternedId, MemberId, ModuleId, ScopeId, SourceRegionId, SymbolId, TypeId},
    intern::Intern,
};

use crate::{
    lookup::scopes::scopes_concepts::{
        AssociatedScopeKind, SCOPE_NEST_ONLY, SCOPE_VAR_ONLY, ScopeInfo, ScopeLookupPattern,
        ScopeLookupPreferenceFlags, ScopeType, SymbolLookupOutput,
    },
    script_compiler::ScriptCompiler,
    semantic::hir::{
        hir_concepts::Type,
        hir_symbols::{MemberSymbolKind, Symbol, SymbolKind, VariableState},
    },
};

/// Locally searches for the given name id. Locally searching in this context means solely
/// searching the scope given for the identifier due to parent relationships not existing.
pub fn find_sym_id_local(
    compiler: &ScriptCompiler,
    scope_id: ScopeId,
    target_name_id: InternedId,
) -> Option<SymbolId> {
    // There are no parent hierarchiable (Is this a word?) language semantics yet other than single
    // local scopes so this is just a single scope search.
    let local_scope = &compiler.scopes[scope_id].scope;

    for (current_name_id, current_sym_id) in &local_scope.table.interned_to_sym {
        if *current_name_id == target_name_id {
            return Some(*current_sym_id);
        }
    }

    None
}

//NOTE: Exists for separation reasons due to the compiler becoming bloated in many forms
/// Get's `TypeId` associated with the `InternedId` given if possible
pub fn find_type_id(
    compiler: &ScriptCompiler,
    owner_id: ModuleId,
    target_name_id: InternedId,
    scope_type: ScopeType,
    lookup_pat: ScopeLookupPattern,
) -> Option<TypeId> {
    let current_mod = &compiler.mods[owner_id];
    let accessible_scopes = compute_accessible_scopes(
        lookup_pat,
        scope_type.accessible_scopes(),
        current_mod.region_id,
    );
    // Loops over all allowed scopes and checks their individual namespaces

    for allowed_scope_type in accessible_scopes.iter().copied() {
        // In this scenario the scope may or may not exist since this could be used from
        // another module
        if let Some(scope_info) =
            find_scope_in_mod(compiler, allowed_scope_type, current_mod.mod_id)
        {
            //NOTE: Make sure I work
            if let Some(current_sym_id) = scope_info
                .scope
                .table
                .interned_to_sym
                .get(&target_name_id)
                .copied()
            {
                match &compiler.symbols[current_sym_id].kind {
                    SymbolKind::Type(type_id) => return Some(*type_id),
                    SymbolKind::Variable(var_id) => {
                        let type_id = match compiler.variables[*var_id].state {
                            VariableState::ReservedTypeSlot(type_id) => type_id,
                            VariableState::Known(val_id) => compiler.values[val_id].type_id,
                        };

                        return Some(type_id);
                    }
                    SymbolKind::ExternType | SymbolKind::Namespace | SymbolKind::Directive(_) => {
                        return None;
                    }
                }
            }
        }
    }

    None
}

/// Searches the given module for the given `ScopeType` by iterating through it's scopes
/// and returns `Some` if it's found, `None` otherwise.
pub fn find_scope_in_mod(
    compiler: &ScriptCompiler,
    target: ScopeType,
    owner_id: ModuleId,
) -> Option<&ScopeInfo> {
    let mod_owner = &compiler.mods[owner_id];
    for scope_id in &mod_owner.scopes {
        let scope_info = &compiler.scopes[*scope_id];
        if scope_info.scope.scope_type == target {
            return Some(scope_info);
        }
    }

    None
}

//TEST:
/// Targets only intrinsic scopes when searching.
pub fn find_sym_id_intrinsic(
    compiler: &ScriptCompiler,
    associated_scope: AssociatedScopeKind,
    target_name_id: InternedId,
    scope_type: ScopeType,
) -> Option<SymbolLookupOutput> {
    // Is this ok?
    let lookup_pat = ScopeLookupPattern::NamespaceOnly;

    // Avoiding vector allocations right now so it can just use a pointer offset instead based off
    // of hard-coded truths but will probably just, not do that.
    match associated_scope {
        AssociatedScopeKind::Module(mod_id) => {
            let current_mod = &compiler.mods[mod_id];
            let accessible_scopes = compute_accessible_scopes(
                lookup_pat,
                scope_type.accessible_scopes(),
                current_mod.region_id,
            );

            for allowed_scope_type in accessible_scopes {
                if let Some(scope_info) = find_scope_in_mod(compiler, *allowed_scope_type, mod_id) {
                    //TODO: Make sure this works as intended
                    if let Some(intrinsic_scope_id) = scope_info.scope.intrinsic_scope {
                        let intrinsic_scope = &compiler.scopes[intrinsic_scope_id].scope;

                        if scope_type == *allowed_scope_type {
                            if let Some(sym_id) = intrinsic_scope
                                .table
                                .interned_to_sym
                                .get(&target_name_id)
                                .copied()
                            {
                                let associated = compiler.symbols[sym_id].associated_scope;
                                return Some(SymbolLookupOutput::new(
                                    sym_id,
                                    scope_info.scope.scope_id,
                                ));
                            }
                        }
                    }
                }
            }

            // If no preferences are matched, returns said default, returning `None` if no default
            // was found
            return None;
        }
        AssociatedScopeKind::Scope(scope_id) => {
            let scope = &compiler.scopes[scope_id].scope;
            if let Some(intrinsic_scope_id) = scope.intrinsic_scope {
                let intrinsic_scope = &compiler.scopes[intrinsic_scope_id].scope;

                if let Some(sym_id) = intrinsic_scope.table.interned_to_sym.get(&target_name_id) {
                    return Some(SymbolLookupOutput::new(*sym_id, intrinsic_scope_id));
                }
            }
        }
    }

    None
}

/// - compiler: The environment to seaerch in
/// - associated_scope: The type of scope to search which could differ depending on if the scope
/// belongs to a module, symbol, etc.
/// - target_name_id: The identifier to search for in the given scope
/// - scope_type: The type of scope this search was started from
/// - lookup_pat: How much access the lookup should have
/// - lookup_pref: Symbols marked as preferred are prioritized as the return type. If not found,
/// returns the last symbol under the target identifier.
///
/// - On `Some`: Returns Symbol found and the `ScopeId` from the scope it was found in
/// - Returns `None` when no symbol with the target identifier was found under the constraints
/// given.
pub fn find_sym_id(
    compiler: &ScriptCompiler,
    associated_scope: AssociatedScopeKind,
    target_name_id: InternedId,
    scope_type: ScopeType,
    lookup_pat: ScopeLookupPattern,
    lookup_pref: ScopeLookupPreferenceFlags,
    // Named struct maybe
) -> Option<SymbolLookupOutput> {
    // Avoiding vector allocations right now so it can just use a pointer offset instead based off
    // of hard-coded truths but will probably just, not do that.
    match associated_scope {
        AssociatedScopeKind::Module(mod_id) => {
            let current_mod = &compiler.mods[mod_id];

            let accessible_scopes = compute_accessible_scopes(
                lookup_pat,
                scope_type.accessible_scopes(),
                current_mod.region_id,
            );

            // If a preferred is given, the most recent same ident symbol found that is not
            // preferred is stored so that it can be returned if the preferred symbol was never found.
            // Compromise!
            let mut default_return: Option<SymbolLookupOutput> = None;

            for allowed_scope_type in accessible_scopes {
                if let Some(scope_info) = find_scope_in_mod(compiler, *allowed_scope_type, mod_id) {
                    // Found a symbol in the particular scope under the given identifier
                    if let Some(sym_id) = scope_info
                        .scope
                        .table
                        .interned_to_sym
                        .get(&target_name_id)
                        .copied()
                    {
                        // Storing what would be the default return if preferences aren't matched
                        default_return =
                            Some(SymbolLookupOutput::new(sym_id, scope_info.scope.scope_id));

                        // Only looping again if not matched
                        let flat = compiler.symbols[sym_id].kind.to_flat();
                        if !lookup_pref.is_preferred(flat) {
                            continue;
                        };

                        // Preferred symbol found
                        return default_return;
                    }

                    //TODO: Make sure this works as intended
                    if let Some(intrinsic_scope_id) = scope_info.scope.intrinsic_scope {
                        let intrinsic_scope = &compiler.scopes[intrinsic_scope_id].scope;

                        // So if in override, but searching complex, it will not try to look at the
                        // intrinsic scope unless it's looking at it's own scope
                        //
                        // Further meaning: Any scope can be given an intrinsic scope. But, it is
                        // only semantically viable to look inside of that intrinsic scope if the
                        // current scope is related. So, if our current scope is complex and we see
                        // an intrinsic scope for override, this disallows that search because
                        // complex should not allow override intrinsic symbols to be used inside complex.
                        if scope_type == *allowed_scope_type {
                            if let Some(sym_id) = intrinsic_scope
                                .table
                                .interned_to_sym
                                .get(&target_name_id)
                                .copied()
                            {
                                //WARN: This is a bit dangerous since it kinda depends on preference
                                //managers having this exact code.
                                let flat = compiler.symbols[sym_id].kind.to_flat();
                                default_return = Some(SymbolLookupOutput::new(
                                    sym_id,
                                    scope_info.scope.scope_id,
                                ));

                                if !lookup_pref.is_preferred(flat) {
                                    continue;
                                }

                                return default_return;
                            }
                        }
                    }
                }
            }

            // If no preferences are matched, returns said default, returning `None` if no default
            // was found
            return default_return;
        }
        AssociatedScopeKind::Scope(scope_id) => {
            let scope = &compiler.scopes[scope_id].scope;
            if let Some(sym_id) = scope.table.interned_to_sym.get(&target_name_id) {
                return Some(SymbolLookupOutput::new(*sym_id, scope_id));
            }

            if let Some(intrinsic_scope_id) = scope.intrinsic_scope {
                let intrinsic_scope = &compiler.scopes[intrinsic_scope_id].scope;

                if let Some(sym_id) = intrinsic_scope.table.interned_to_sym.get(&target_name_id) {
                    return Some(SymbolLookupOutput::new(*sym_id, intrinsic_scope_id));
                }
            }
        }
    }

    None
}

/// Returns `Vec<ScopeType>` of all scope types inside the current module
pub fn collect_mod_scope_types(compiler: &ScriptCompiler, owner_id: ModuleId) -> Vec<ScopeType> {
    // There are O(ScopeType) different scopes allowed in a module so having this be iterative costs nothing
    let module = &compiler.mods[owner_id];
    module
        .scopes
        .iter()
        .map(|s_id| compiler.scopes[*s_id].scope.scope_type)
        .collect()
}

/// Finds all symbols under the given interned identifier and returns similar symbol ids to the
/// given target
pub fn find_symbols_named<'a>(
    compiler: &'a ScriptCompiler,
    target_name_id: InternedId,
    exact_match: bool,
    associated_scope: Option<AssociatedScopeKind>,
    interner: &Intern,
) -> Vec<SymbolId> {
    let mut found_syms: Vec<SymbolId> = Vec::new();
    let target_bytes = interner.search(target_name_id).as_bytes();

    for sym in compiler.symbols.iter() {
        if exact_match {
            if sym.name_id == target_name_id {
                found_syms.push(sym.sym_id);
            }
        } else {
            let sym_bytes = interner.search(sym.name_id).as_bytes();

            if algoc::bytes::is_similar(sym_bytes, target_bytes) {
                found_syms.push(sym.sym_id);
            }
        }
    }

    found_syms
}

/// Finds all symbols under the given string identifier and returns their symbols as references
pub fn find_symbols_named_ref<'a>(
    compiler: &'a ScriptCompiler,
    target_name_id: InternedId,
) -> (Vec<&'a Symbol>, Vec<&'a MemberSymbolKind>) {
    let mut found_syms: Vec<&Symbol> = Vec::new();
    let mut found_members: Vec<&MemberSymbolKind> = Vec::new();

    for sym in &compiler.symbols.items {
        if sym.name_id == target_name_id {
            found_syms.push(&sym);
        }

        match sym.kind {
            SymbolKind::Type(type_id) => {
                let new_members = collect_inner_symbols(compiler, type_id, target_name_id);
                found_members.append(
                    &mut new_members
                        .iter()
                        .map(|m_id| &compiler.sym_members[*m_id])
                        .collect(),
                );
            }
            // To avoid allocating Vec if there is no possible inner
            _ => (),
        }
    }

    (found_syms, found_members)
}

// To avoid O(symbols) search by allowing module specific lookups
/// Finds all symbols under the given string identifier within the specific module given
pub fn find_symbols_named_from_module<'a>(
    compiler: &'a ScriptCompiler,
    interner: &Intern,
    target_mod_id: ModuleId,
    ident: &str,
) -> (Vec<SymbolId>, Vec<&'a MemberSymbolKind>) {
    let target_name_id = match interner.try_search_str(ident) {
        Some(id) => id,
        None => return (Vec::new(), Vec::new()),
    };

    let module = &compiler.mods[target_mod_id];

    let mut found_symbols = Vec::new();
    for scope_id in &module.scopes {
        let scope = &compiler.scopes[*scope_id].scope;
        for (interned_id, sym_id) in &scope.table.interned_to_sym {
            if *interned_id == target_name_id {
                found_symbols.push(*sym_id);
            }
        }
    }

    todo!()
}

/// Finds all symbols under the given string identifier within the specific module given
pub fn find_symbols_named_from_module_ref<'a>(
    compiler: &'a ScriptCompiler,
    interner: &Intern,
    target_mod_id: ModuleId,
    ident: &str,
) -> (Vec<&'a Symbol>, Vec<&'a MemberSymbolKind>) {
    let target_name_id = match interner.try_search_str(ident) {
        Some(id) => id,
        None => return (Vec::new(), Vec::new()),
    };

    let mut found_syms: Vec<&Symbol> = Vec::new();
    let mut found_members: Vec<&MemberSymbolKind> = Vec::new();

    let module = &compiler.mods[target_mod_id];

    for scope_id in &module.scopes {
        let scope = &compiler.scopes[*scope_id].scope;
        for sym_id in scope.table.interned_to_sym.values() {
            let sym = &compiler.symbols[*sym_id];

            if sym.name_id == target_name_id {
                found_syms.push(&sym);
            }

            match sym.kind {
                SymbolKind::Type(type_id) => {
                    let new_members = collect_inner_symbols(compiler, type_id, target_name_id);
                    found_members.append(
                        &mut new_members
                            .iter()
                            .map(|m_id| &compiler.sym_members[*m_id])
                            .collect(),
                    );
                }
                // To avoid allocating Vec if there is no possible inner
                _ => (),
            }
        }
    }

    (found_syms, found_members)
}

fn collect_inner_symbols<'a>(
    compiler: &'a ScriptCompiler,
    type_id: TypeId,
    target_name_id: InternedId,
) -> Vec<MemberId> {
    // Um, 20 mb?
    let mut found: Vec<MemberId> = Vec::new();

    match &compiler.types[type_id].ty {
        Type::Struct(struct_def) => {
            for member_id in &struct_def.fields {
                let field = compiler.get_field(*member_id);
                if field.name_id == target_name_id {
                    found.push(field.member_id);
                }
            }
        }
        Type::Enum(enum_def) => {
            for member_id in &enum_def.variants {
                let variant = compiler.get_variant(*member_id);
                if variant.name_id == target_name_id {
                    found.push(variant.member_id);
                }
            }
        }
        Type::Alias(alias_def) => {
            // Not sure what to do with params yet
            // for thing in &alias_def.params {}
        }
        Type::Deferred(inner_type_id) => found.append(&mut collect_inner_symbols(
            compiler,
            *inner_type_id,
            target_name_id,
        )),
        // Function isn't possible as an inner
        Type::Func(_)
        | Type::TypeDef(_)
        | Type::Boundaries(_)
        | Type::BuiltinTypeInfo(_)
        | Type::Unknown => (),
    }

    found
}

/// Returns accessible scopes, given the lookup pattern and region id.
fn compute_accessible_scopes<'a>(
    lookup_pat: ScopeLookupPattern,
    accessible_scopes: &'a [ScopeType],
    region_id_opt: Option<SourceRegionId>,
) -> &'a [ScopeType] {
    match lookup_pat {
        //WARN: Core is always the last scope so this is kept so an owned vec isn't created
        //May change
        ScopeLookupPattern::NamespaceOnly if region_id_opt.is_some() => {
            &accessible_scopes[..accessible_scopes.len() - 1]
        }
        // If it's core then it'll only have access to core anyways so this is fine
        ScopeLookupPattern::NoRestrictions | ScopeLookupPattern::NamespaceOnly => accessible_scopes,
        ScopeLookupPattern::OnlyVar => &SCOPE_VAR_ONLY,
        ScopeLookupPattern::OnlyNest => &SCOPE_NEST_ONLY,
    }
}
