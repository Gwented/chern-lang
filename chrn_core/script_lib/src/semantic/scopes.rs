use std::fmt::Display;

use chrn_utils::id_types::{InternedId, ModuleId, ScopeId, SymbolId, TypeId};

use crate::{
    script_compiler::ScriptCompiler,
    semantic::representation::{Symbol, SymbolKind, Table},
};

//TODO: Maybe this is the point where the scope wrapper comes in
/// Structure that holds scope data
#[derive(Debug)]
pub struct ScopeInfo {
    pub scope: Scope,
    /// For debugging purposes so that the symbol of origin is known for where a namespace lookup
    /// occured, beyond just the module or scope of origin.
    pub sym_owner: Option<SymbolId>,
    pub mod_owner: ModuleId,
}

impl ScopeInfo {
    pub fn new(scope: Scope, sym_owner: Option<SymbolId>, mod_owner: ModuleId) -> ScopeInfo {
        ScopeInfo {
            scope,
            sym_owner,
            mod_owner,
        }
    }
}

pub const SCOPE_CORE: u8 = 1 << 0;
pub const SCOPE_NEUTRAL: u8 = 1 << 1;
pub const SCOPE_VAR: u8 = 1 << 2;
pub const SCOPE_NEST: u8 = 1 << 3;
pub const SCOPE_COMPLEX: u8 = 1 << 4;
pub const SCOPE_OVERRIDE: u8 = 1 << 5;
pub const SCOPE_LOCAL: u8 = 1 << 6;

// Bitwise into array of scopes that filters each time a lookup is done?
pub static SCOPE_CORE_ACCESSIBLE: [ScopeType; 1] = [ScopeType::Core];
pub static SCOPE_NEUTRAL_ACCESSIBLE: [ScopeType; 2] = [ScopeType::Neutral, ScopeType::Core];

//WARN: Suspicious accessibility
pub static SCOPE_LOCAL_ACCESSIBLE: [ScopeType; 1] = [ScopeType::Local];

pub static SCOPE_REST_ACCESSIBLE: [ScopeType; 5] = [
    ScopeType::Neutral,
    ScopeType::Nest,
    ScopeType::Complex,
    ScopeType::Override,
    ScopeType::Core,
];

// Neutral, var, nest, and complex scopes can only access variables from neutral and nest.
// Override is unsure
#[derive(Debug)]
pub struct Scope {
    pub table: Table,
    pub scope_id: ScopeId,
    pub scope_type: ScopeType,
    pub intrinsic_scope: Option<ScopeId>,
    pub is_intrinsic: bool,
    pub accessible_scopes: &'static [ScopeType],
}

impl Scope {
    pub(crate) fn new(
        scope_id: ScopeId,
        scope_type: ScopeType,
        is_intrinsic: bool,
        intrinsic_scope: Option<ScopeId>,
    ) -> Scope {
        let accessible_scopes = scope_type.accessible_scopes();
        Scope {
            table: Table::new(),
            scope_id,
            scope_type,
            intrinsic_scope,
            accessible_scopes,
            is_intrinsic,
            // pub visible_scopes: Vec<ScopeId>,
        }
    }

    pub(crate) fn with_table(
        scope_id: ScopeId,
        scope_type: ScopeType,
        intrinsic_scope: Option<ScopeId>,
        is_intrinsic: bool,
        table: Table,
    ) -> Scope {
        let accessible_scopes = scope_type.accessible_scopes();
        Scope {
            table,
            scope_id,
            scope_type,
            is_intrinsic,
            intrinsic_scope,
            accessible_scopes,
        }
    }
}
/// Locally searches for the given name id. Locally searching in this context means solely
/// searching the scope given for the identifier due to parent relationships not existing.
pub fn get_sym_id_local(
    compiler: &ScriptCompiler,
    scope_id: ScopeId,
    target_name_id: InternedId,
) -> Option<SymbolId> {
    // There are no parent hierarchiable (Is this a word?) language semantics yet other than single
    // local scopes so this is just a single scope search.
    let local_scope = &compiler.scopes[scope_id.id as usize].scope;

    for (current_name_id, current_sym_id) in &local_scope.table.interned_to_sym {
        if *current_name_id == target_name_id {
            return Some(*current_sym_id);
        }
    }

    None
}

//NOTE: Exists for separation reasons due to the compiler becoming bloated in many forms
/// Get's `TypeId` associated with the `InternedId` given if possible
pub fn get_type_id(
    compiler: &ScriptCompiler,
    owner_id: ModuleId,
    target_name_id: InternedId,
    scope_type: ScopeType,
    lookup_pattern: LookupPattern,
) -> Option<TypeId> {
    let current_mod = &compiler.mods[owner_id.id];
    //WARN: Core is always the last scope so this is kept so an owned vec isn't created
    //May change
    let accessible_scopes = scope_type.accessible_scopes();
    let accessible_scopes = match lookup_pattern {
        LookupPattern::NamespaceOnly if current_mod.src_metadata.is_some() => {
            &accessible_scopes[..accessible_scopes.len() - 1]
        }
        // If it's core then it'll only have access to core anyways so this is fine
        LookupPattern::NoRestrictions | LookupPattern::NamespaceOnly => accessible_scopes,
    };
    // I don't think this can fail. Should maybe expect for clarity.
    //     let scope = &compiler.scopes[scope_id.id].scope;
    // Loops over all allowed scopes and checks their individual namespaces

    for allowed_scope_type in accessible_scopes.iter().copied() {
        // In this scenario the scope may or may not exist since this could be used from
        // another module
        if let Some(scope_info) = compiler.find_scope(allowed_scope_type, current_mod.mod_id) {
            for current_sym_id in scope_info.scope.table.interned_to_sym.values() {
                let current_sym = &compiler.symbols[current_sym_id.id as usize];
                if current_sym.name_id == target_name_id {
                    match &compiler.symbols[current_sym_id.id as usize].kind {
                        SymbolKind::Type(type_id) => return Some(*type_id),
                        // Is this even possible
                        SymbolKind::Val(val_id) => {
                            return Some(compiler.values[val_id.id as usize].type_id);
                        }
                        SymbolKind::ReservedTypeSlot(type_id) => return Some(*type_id),
                        SymbolKind::Module(_) => return None,
                    }
                }
            }
        }
    }

    None
}

pub fn find_scope(
    compiler: &ScriptCompiler,
    scope_type: ScopeType,
    owner_id: ModuleId,
) -> Option<&ScopeInfo> {
    let mod_owner = &compiler.mods[owner_id.id];
    for scope_id in &mod_owner.scopes {
        let scope_info = &compiler.scopes[scope_id.id];
        if scope_info.scope.scope_type == scope_type {
            return Some(scope_info);
        }
    }

    None
}

// TEST:
/// - compiler: The environment to seaerch in
/// - associated_scope: The type of scope to search which could differ depending on if the scope
/// belongs to a module, symbol, etc.
/// - target_name_id: The identifier to search for in the given scope
/// - scope_type: The type of scope this search was started from
/// - lookup_pattern: How much access the lookup should have
pub fn get_sym_id(
    compiler: &ScriptCompiler,
    associated_scope: AssociatedScopeKind,
    target_name_id: InternedId,
    scope_type: ScopeType,
    lookup_pattern: LookupPattern,
) -> Option<SymbolId> {
    // Avoiding vector allocations right now so it can just use a pointer offset instead based off
    // of hard-coded truths but will probably just, not do that.
    //TEST:
    match associated_scope {
        AssociatedScopeKind::Module(mod_id) => {
            let current_mod = &compiler.mods[mod_id.id];

            let accessible_scopes = scope_type.accessible_scopes();
            let accessible_scopes = match lookup_pattern {
                LookupPattern::NamespaceOnly if current_mod.src_metadata.is_some() => {
                    &accessible_scopes[..accessible_scopes.len() - 1]
                }
                // If it's core then it'll only have access to core anyways so this is fine
                LookupPattern::NoRestrictions | LookupPattern::NamespaceOnly => accessible_scopes,
            };

            for allowed_scope_type in accessible_scopes.iter() {
                if let Some(scope_info) = compiler.find_scope(*allowed_scope_type, mod_id) {
                    if let Some(sym_id) =
                        scope_info.scope.table.interned_to_sym.get(&target_name_id)
                    {
                        return Some(*sym_id);
                    }

                    //TODO: Make sure this works as intended
                    if let Some(intrinsic_scope_id) = scope_info.scope.intrinsic_scope {
                        let intrinsic_scope = &compiler.scopes[intrinsic_scope_id.id].scope;

                        // So if in override, but searching complex, it will not try to look at the
                        // intrinsic scope unless it's looking at it's own scope
                        if scope_type == *allowed_scope_type {
                            if let Some(sym_id) =
                                intrinsic_scope.table.interned_to_sym.get(&target_name_id)
                            {
                                return Some(*sym_id);
                            }
                        }
                    }
                }
            }
        }
        AssociatedScopeKind::Scope(scope_id) => {
            let scope = &compiler.scopes[scope_id.id].scope;
            if let Some(sym_id) = scope.table.interned_to_sym.get(&target_name_id) {
                return Some(*sym_id);
            }

            //TODO: Make sure this works as intended
            if let Some(intrinsic_scope_id) = scope.intrinsic_scope {
                let intrinsic_scope = &compiler.scopes[intrinsic_scope_id.id].scope;

                if let Some(sym_id) = intrinsic_scope.table.interned_to_sym.get(&target_name_id) {
                    return Some(*sym_id);
                }
            }
        }
    }

    None
}

/// Enum representing all kinds of scopes usable in chrn
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ScopeType {
    Core,
    Local,
    Neutral,
    Var,
    Nest,
    Complex,
    Override,
}

impl ScopeType {
    /// Direct representation of how the language views scope accessibility.
    /// `needs_global` purely exists for all scope accessibility reasons
    pub fn accessible_scopes(self) -> &'static [ScopeType] {
        match self {
            ScopeType::Core => &SCOPE_CORE_ACCESSIBLE,
            // Mainly for internal usage, not an actual program recognizable scope
            // Neutral can only access neutral because this section is purely for declaring and
            // using in other sections
            ScopeType::Neutral => &SCOPE_NEUTRAL_ACCESSIBLE,
            ScopeType::Var | ScopeType::Nest | ScopeType::Complex | ScopeType::Override => {
                &SCOPE_REST_ACCESSIBLE
            }
            ScopeType::Local => &SCOPE_LOCAL_ACCESSIBLE,
        }
    }

    pub(crate) fn to_u8(self) -> u8 {
        match self {
            ScopeType::Core => SCOPE_CORE,
            ScopeType::Neutral => SCOPE_NEUTRAL,
            ScopeType::Var => SCOPE_VAR,
            ScopeType::Nest => SCOPE_NEST,
            ScopeType::Complex => SCOPE_COMPLEX,
            ScopeType::Override => SCOPE_OVERRIDE,
            ScopeType::Local => SCOPE_LOCAL,
        }
    }

    pub(crate) fn has_intrinsic_scope(self) -> bool {
        match self {
            ScopeType::Complex | ScopeType::Override => true,
            ScopeType::Core
            | ScopeType::Local
            | ScopeType::Neutral
            | ScopeType::Nest
            | ScopeType::Var => false,
        }
    }
}

/// This enum is intended to disallow core defined values from being searched for when syntax such
/// as "main.i32" is used. i32 is not owned by main, but innately main is attached to core, meaning
/// without the explicit noting of whether we are searching a singular module's namespace it would
/// innately allow for main.i32 to be interpreted the same as if just i32 was written, which is
/// wrong since the namespace "main" owns no such thing.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LookupPattern {
    /// Applies no restriction to lookups. Meaning, core is automatically searched since it's
    /// intrinsic, any scope's accessible scopes can be searched with no restriction.
    NoRestrictions,
    // WHat
    /// Restricts lookup to only search what is within the given namespace, which restricts modules
    /// such as core, or anything not declared within the symbol's scope containment?
    NamespaceOnly,
}

// TODO: Formattable
impl Display for ScopeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScopeType::Core => write!(f, "core"),
            ScopeType::Neutral => write!(f, "neutral"),
            ScopeType::Var => write!(f, "var"),
            ScopeType::Nest => write!(f, "nest"),
            ScopeType::Complex => write!(f, "complex"),
            ScopeType::Override => write!(f, "override"),
            ScopeType::Local => write!(f, "local"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Scope type specifically for if a symbol has an associated scope tied to it
pub enum AssociatedScopeKind {
    // A bit redundant since odules already hold themselves as a scope
    /// Meaning the scope is inside of a module's vector of `ScopeId`
    Module(ModuleId),
    /// Meaning the scope is just attached to a symbol's namespace
    Scope(ScopeId),
}

pub struct IntrinsicRegistry {
    pub core_mod_id: ModuleId,
    pub complex: Option<ScopeId>,
    pub overrid: Option<ScopeId>,
}

impl IntrinsicRegistry {
    pub fn new(
        core_mod_id: ModuleId,
        complex: Option<ScopeId>,
        overrid: Option<ScopeId>,
    ) -> IntrinsicRegistry {
        IntrinsicRegistry {
            core_mod_id,
            complex,
            overrid,
        }
    }
}
