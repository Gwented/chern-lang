use std::fmt::Display;

use chrn_utils::{
    builtins::BuiltinType,
    id_types::{InternedId, ModuleId, ScopeId, SymbolId, TypeId},
};

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

// Bitwise food
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
    //FIX: BITWISE FOOD LATER
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
    // I don't think this can fail. Should maybe expect for clarity.
    // Loops over all allowed scopes and checks their individual namespaces
    for scope_id in &current_mod.scopes {
        let scope = &compiler.scopes[scope_id.id].scope;
        // In this scenario the scope may or may not exist since this could be used from
        // another module
        for allowed_scope_type in scope.accessible_scopes.iter().copied() {
            if let Some(scope_info) = compiler.find_scope(allowed_scope_type, current_mod) {
                for (current_ast_id, current_name_id) in &scope_info.scope.table.name_ids {
                    if *current_name_id == target_name_id {
                        let scope_id =
                            compiler.extract_scope_id(allowed_scope_type, current_mod.mod_id);
                        let scope_info = compiler.get_scope(scope_id);

                        let sym_id = scope_info.scope.table.ast_to_sym[&current_ast_id];
                        return Some(sym_id);
                    }
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
    // I don't think this can fail. Should maybe expect for clarity.
    //     let scope = &compiler.scopes[scope_id.id].scope;
    // Loops over all allowed scopes and checks their individual namespaces

    for scope_id in &current_mod.scopes {
        let scope = &compiler.scopes[scope_id.id].scope;
        dbg!(scope);

        for allowed_scope_type in scope.accessible_scopes.iter().copied() {
            // In this scenario the scope may or may not exist since this could be used from
            // another module
            if let Some(scope_info) = compiler.find_scope(allowed_scope_type, current_mod) {
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
    }

    todo!("No core found");
    // Uhhhhhhh brain isn't working
    if let Some(ty) = BuiltinType::try_from_interned_id(target_name_id.id) {
        // This technically relies on the original pushing of values being in order so
        // may also be changed to a const idx but fine for iteration purposes
        return Some(TypeId::new(ty.kind() as u32));
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

    // pub(crate) fn to_u8(&self) -> u8 {
    //     match self {
    //         ScopeType::Core => todo!(),
    //         ScopeType::Neutral => todo!(),
    //         ScopeType::Var => todo!(),
    //         ScopeType::Nest => todo!(),
    //         ScopeType::Complex => todo!(),
    //         ScopeType::Override => todo!(),
    //     }
    // }
}

// TODO: Formattable
impl Display for ScopeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScopeType::Core => write!(f, "intrinsic"),
            ScopeType::Neutral => write!(f, "neutral"),
            ScopeType::Var => write!(f, "var"),
            ScopeType::Nest => write!(f, "nest"),
            ScopeType::Complex => write!(f, "complex"),
            ScopeType::Override => write!(f, "override"),
        }
    }
}
