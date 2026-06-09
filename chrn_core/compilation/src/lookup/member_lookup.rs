use chrn_utils::id_types::{InternedId, MemberId, SymbolId, TypeId};

use crate::{
    script_compiler::ScriptCompiler,
    semantic::hir::{SymbolKind, Type},
};

//TEST:
#[derive(Debug)]
pub enum MemberLookupResult {
    Found(MemberId),
    /// Example: A module does not have members as a field would
    TypeHasNoMembers(TypeId),
    /// Example: A type having members, but not having the field identifier specified
    TypeDoesNotContainMember(TypeId),
    /// Example: A type having members, but not having the field identifier specified
    SymbolHasNoMembers,
    Unknown,
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
        SymbolKind::Val(val_id) => {
            let val_type_id = compiler.values[val_id.id as usize].type_id;
            lookup_field_inner(compiler, val_type_id, target_name_id)
        }
        SymbolKind::ReservedTypeSlot(_) => MemberLookupResult::Unknown,
        SymbolKind::Config(_) | SymbolKind::Module(_) => MemberLookupResult::SymbolHasNoMembers,
    }
}

fn lookup_field_inner(
    compiler: &ScriptCompiler,
    type_id: TypeId,
    target_name_id: InternedId,
) -> MemberLookupResult {
    match &compiler.types[type_id.id as usize].ty {
        Type::BuiltinType(builtin_type) => {
            todo!("Not sure yet")
        }
        Type::Struct(struct_def) => {
            for member_id in &struct_def.fields {
                let field = compiler.get_field(*member_id);
                if field.name_id == target_name_id {
                    return MemberLookupResult::Found(field.member_id);
                }
            }

            MemberLookupResult::TypeDoesNotContainMember(type_id)
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
        Type::TypeDef(_) => MemberLookupResult::TypeHasNoMembers(type_id),
        Type::Constrained(type_constraint_flags) => todo!(),
        // WARN: DANGEROUS
        Type::Deferred(inner_type_id) => {
            lookup_field_inner(compiler, *inner_type_id, target_name_id)
        }
        Type::Unknown => MemberLookupResult::Unknown,
    }
}
