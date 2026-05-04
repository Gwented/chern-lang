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

// Neutral, var, nest, and complex scopes can only access variables from neutral and nest.
// Override is unsure
#[derive(Debug)]
pub struct Scope {
    pub table: Table,
    pub scope_id: ScopeId,
    pub scope_type: ScopeType,
}

impl Scope {
    pub(crate) fn new(scope_id: ScopeId, scope_type: ScopeType) -> Scope {
        Scope {
            table: Table::new(),
            scope_id,
            scope_type,
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
    let allowed_scopes = scope_type.accessible_scopes();

    // Loops over all allowed scopes and checks their individual namespaces
    for allowed_scope_type in allowed_scopes {
        // In this scenario the scope may or may not exist since this could be used from
        // another module
        if let Some(scope) = current_mod.find_scope(allowed_scope_type) {
            for (current_ast_id, current_name_id) in &scope.table.name_ids {
                if *current_name_id == target_name_id {
                    let scope_id = current_mod.extract_scope_id(allowed_scope_type);
                    let scope = current_mod.get_scope(scope_id);

                    let sym_id = scope.table.sym_ids[&current_ast_id];
                    return Some(sym_id);
                }
            }
        }
    }

    //TODO: No std symbols yet
    // let std_mod = &compiler.mods[compiler.std_mod_id.id];

    //TEST: If all scopes fail

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
    let allowed_scopes = scope_type.accessible_scopes();

    // Loops over all allowed scopes and checks their individual namespaces
    for allowed_scope_type in allowed_scopes {
        // In this scenario the scope may or may not exist since this could be used from
        // another module
        if let Some(scope) = current_mod.find_scope(allowed_scope_type) {
            for (current_ast_id, current_name_id) in &scope.table.name_ids {
                if *current_name_id == target_name_id {
                    let scope_id = current_mod.extract_scope_id(allowed_scope_type);
                    let scope = current_mod.get_scope(scope_id);

                    let sym_id = scope.table.sym_ids[&current_ast_id];
                    match &compiler.symbols[&sym_id].kind {
                        SymbolKind::Type(type_id) => return Some(*type_id),
                        SymbolKind::Val(val_id) => {
                            return Some(compiler.values[val_id.id as usize].type_id);
                        }
                        SymbolKind::Unknown => return None,
                    }
                }
            }
        }
    }

    // Uhhhhhhh brain isn't working
    if let Some(ty) = BuiltinType::try_from_interned_id(target_name_id.id) {
        // This technically relies on the original pushing of values being in order so
        // may also be changed to a const idx but fine for iteration purposes
        return Some(TypeId::new(ty.kind() as u32));
    }

    None
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ScopeType {
    Intrinsic,
    Neutral,
    Var,
    Nest,
    Complex,
    Override,
}

impl ScopeType {
    /// Direct representation of how the language views scope accessibility.
    /// `needs_global` purely exists for all scope accessibility reasons
    pub(crate) fn accessible_scopes(&self) -> Vec<ScopeType> {
        match self {
            ScopeType::Intrinsic => vec![ScopeType::Intrinsic],
            // Mainly for internal usage, not an actual program recognizable scope
            // Neutral can only access neutral because this section is purely for declaring and
            // using in other sections
            ScopeType::Neutral => vec![ScopeType::Neutral],
            ScopeType::Var | ScopeType::Nest | ScopeType::Complex => {
                vec![ScopeType::Neutral, ScopeType::Nest]
            }
            ScopeType::Override => vec![ScopeType::Nest],
        }
    }
}

// TODO: Formattable
impl Display for ScopeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScopeType::Intrinsic => write!(f, "intrinsic"),
            ScopeType::Neutral => write!(f, "neutral"),
            ScopeType::Var => write!(f, "var"),
            ScopeType::Nest => write!(f, "nest"),
            ScopeType::Complex => write!(f, "complex"),
            ScopeType::Override => write!(f, "override"),
        }
    }
}
