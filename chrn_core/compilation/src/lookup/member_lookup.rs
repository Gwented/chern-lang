use chrn_utils::id_types::{InternedId, MemberId, SymbolId, TypeId};

use crate::{
    script_compiler::ScriptCompiler,
    semantic::hir::{SymbolKind, Type, VariableState},
};

//TEST:
/// Result type for member lookups. This exists due to the fact that there is no `Ok` or `Err`
/// inherit concept behind whether or not something was found.
#[derive(Debug)]
pub enum MemberLookupResult {
    Found(MemberId),
    /// A module does not have members as a field would
    InvalidTypeMemberAccess(TypeId),
    /// A type having members, but not having the field identifier specified
    MemberNotFoundInType(TypeId),
    /// A symbol not having the capability of holding members at the language level
    InvalidSymbolMemberAccess,
    Unknown(TypeId),
}

pub fn collect_all_members(compiler: &ScriptCompiler, sym_id: SymbolId) -> MemberLookupResult {
    todo!()
}

// Naming has a little collision since member runtime lookup has the same name as this,
// realistically, const lookup.
pub fn lookup_member(
    compiler: &ScriptCompiler,
    sym_id: SymbolId,
    target_name_id: InternedId,
) -> MemberLookupResult {
    match compiler.symbols[sym_id.id as usize].kind {
        SymbolKind::Type(inner_type_id) => {
            lookup_field_inner(compiler, inner_type_id, target_name_id)
        }
        SymbolKind::Variable(var_id) => match compiler.variables[var_id.id as usize].state {
            VariableState::ReservedTypeSlot(type_id) => MemberLookupResult::Unknown(type_id),
            VariableState::Known(val_id) => {
                let val_type_id = compiler.values[val_id.id as usize].type_id;
                lookup_field_inner(compiler, val_type_id, target_name_id)
            }
        },
        SymbolKind::Config(_) | SymbolKind::Module(_) => {
            MemberLookupResult::InvalidSymbolMemberAccess
        }
    }
}

fn lookup_field_inner(
    compiler: &ScriptCompiler,
    type_id: TypeId,
    target_name_id: InternedId,
) -> MemberLookupResult {
    match &compiler.types[type_id.id as usize].ty {
        Type::BuiltinType(builtin_ty) => {
            // Members/Methods do not exist for types yet
            MemberLookupResult::InvalidTypeMemberAccess(type_id)
        }
        Type::Struct(struct_def) => {
            for member_id in &struct_def.fields {
                let field = compiler.get_field(*member_id);
                if field.name_id == target_name_id {
                    return MemberLookupResult::Found(field.member_id);
                }
            }

            MemberLookupResult::MemberNotFoundInType(type_id)
        }
        // But enums aren't fields..they're namespaces
        Type::Enum(enum_def) => {
            for member_id in &enum_def.variants {
                let variant = compiler.get_field(*member_id);
                if variant.name_id == target_name_id {
                    return MemberLookupResult::Found(variant.member_id);
                }
            }

            todo!()
        }
        Type::Alias(alias_def) => todo!(),
        Type::Func(func_def) => todo!(),
        Type::TypeDef(_) => MemberLookupResult::InvalidTypeMemberAccess(type_id),
        Type::Constrained(type_constraint_flags) => todo!(),
        // WARN: DANGEROUS
        Type::Deferred(inner_type_id) => {
            lookup_field_inner(compiler, *inner_type_id, target_name_id)
        }
        Type::Unknown => MemberLookupResult::Unknown(type_id),
    }
}
