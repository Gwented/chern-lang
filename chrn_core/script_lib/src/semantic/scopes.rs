use std::fmt::Display;

use chrn_utils::id_types::{InternedId, ModuleId, ScopeId, SymbolId, TypeId};

use crate::{
    modules::Module,
    script_compiler::ScriptCompiler,
    semantic::representation::{SymbolKind, Table},
};

//TODO: Maybe this is the point where the scope wrapper comes in
#[derive(Debug)]
pub struct ScopeInfo {
    pub scope: Scope,
    pub owner: ModuleId,
}

impl ScopeInfo {
    pub fn new(scope: Scope, owner: ModuleId) -> ScopeInfo {
        ScopeInfo { scope, owner }
    }
}

pub const SCOPE_CORE: u8 = 1 << 0;
pub const SCOPE_NEUTRAL: u8 = 1 << 1;
pub const SCOPE_VAR: u8 = 1 << 2;
pub const SCOPE_NEST: u8 = 1 << 3;
pub const SCOPE_COMPLEX: u8 = 1 << 4;
pub const SCOPE_OVERRIDE: u8 = 1 << 5;

// Bitwise into array of scopes that filters each time a lookup is done?
pub static SCOPE_CORE_ACCESSIBLE: [ScopeType; 1] = [ScopeType::Core];
pub static SCOPE_NEUTRAL_ACCESSIBLE: [ScopeType; 2] = [ScopeType::Neutral, ScopeType::Core];
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
    //FIX: Ok this is not bit-wise food I am scared
    pub accessible_scopes: &'static [ScopeType],
}

impl Scope {
    pub(crate) fn new(scope_id: ScopeId, scope_type: ScopeType) -> Scope {
        let accessible_scopes = scope_type.accessible_scopes();
        Scope {
            table: Table::new(),
            scope_id,
            scope_type,
            accessible_scopes,
            // pub visible_scopes: Vec<ScopeId>,
        }
    }

    pub(crate) fn with_table(scope_id: ScopeId, scope_type: ScopeType, table: Table) -> Scope {
        let accessible_scopes = scope_type.accessible_scopes();
        Scope {
            table,
            scope_id,
            scope_type,
            accessible_scopes,
        }
    }
}

//TEST:
pub fn get_sym_id(
    compiler: &ScriptCompiler,
    current_mod: &Module,
    target_name_id: InternedId,
    scope_type: ScopeType,
) -> Option<SymbolId> {
    let accessible_scopes = scope_type.accessible_scopes();

    for allowed_scope_type in accessible_scopes.iter().copied() {
        if let Some(scope_info) = compiler.find_scope(allowed_scope_type, current_mod.mod_id) {
            for (current_name_id, current_sym_id) in &scope_info.scope.table.interned_to_sym {
                if *current_name_id == target_name_id {
                    return Some(*current_sym_id);
                }
            }
        }
    }

    //TODO: No std symbols yet
    // let std_mod = &compiler.mods[compiler.std_mod_id.id];

    //TEST: If all scopes fail
    todo!("Stop it");

    None
}

//TEST:
/// Get's `TypeId` associated with the `NameId` given if possible. Searches local scope then std
pub fn get_type_id(
    compiler: &ScriptCompiler,
    current_mod: &Module,
    target_name_id: InternedId,
    scope_type: ScopeType,
) -> Option<TypeId> {
    let accessible_scopes = scope_type.accessible_scopes();
    // I don't think this can fail. Should maybe expect for clarity.
    //     let scope = &compiler.scopes[scope_id.id].scope;
    // Loops over all allowed scopes and checks their individual namespaces

    for allowed_scope_type in accessible_scopes.iter().copied() {
        // In this scenario the scope may or may not exist since this could be used from
        // another module
        if let Some(scope_info) = compiler.find_scope(allowed_scope_type, current_mod.mod_id) {
            for current_sym_id in scope_info.scope.table.interned_to_sym.values() {
                let current_sym = &compiler.symbols[current_sym_id];
                if current_sym.name_id == target_name_id {
                    match &compiler.symbols[&current_sym_id].kind {
                        SymbolKind::Type(type_id) => return Some(*type_id),
                        // Is this even possible
                        SymbolKind::Val(val_id) => {
                            return Some(compiler.values[val_id.id as usize].type_id);
                        }
                        SymbolKind::Unknown => return None,
                    }
                }
            }
        }
    }

    None
}

// Bit flags?
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ScopeType {
    Core,
    Neutral,
    Var,
    Nest,
    Complex,
    Override,
}

impl ScopeType {
    /// Direct representation of how the language views scope accessibility.
    /// `needs_global` purely exists for all scope accessibility reasons
    pub(crate) fn accessible_scopes(&self) -> &'static [ScopeType] {
        match self {
            ScopeType::Core => &SCOPE_CORE_ACCESSIBLE,
            // Mainly for internal usage, not an actual program recognizable scope
            // Neutral can only access neutral because this section is purely for declaring and
            // using in other sections
            ScopeType::Neutral => &SCOPE_NEUTRAL_ACCESSIBLE,
            ScopeType::Var | ScopeType::Nest | ScopeType::Complex | ScopeType::Override => {
                &SCOPE_REST_ACCESSIBLE
            }
        }
    }

    pub(crate) fn to_u8(&self) -> u8 {
        match self {
            ScopeType::Core => SCOPE_CORE,
            ScopeType::Neutral => SCOPE_NEUTRAL,
            ScopeType::Var => SCOPE_VAR,
            ScopeType::Nest => SCOPE_NEST,
            ScopeType::Complex => SCOPE_COMPLEX,
            ScopeType::Override => SCOPE_OVERRIDE,
        }
    }
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
        }
    }
}
