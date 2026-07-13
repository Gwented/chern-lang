use chrn_utils::{
    id_types::{InternedId, MemberId, SymbolId, TypeId},
    loop_abort,
};

use crate::{script_compiler::ScriptCompiler, semantic::hir::hir_concepts::Type};

pub enum MemberScopeLookupPattern {
    DotMember,
    StaticMember,
    NoRestrictions,
}

/// Result type for member lookups. This exists due to the fact that there is no `Ok` or `Err`
/// inherit concept behind whether or not something was found.
#[derive(Debug)]
pub enum MemberLookupResult {
    /// `MemberId` found with no issues
    Found(MemberId),
    /// A type that does not have members
    ImpossibleTypeMemberAccess(TypeId),
    /// A type having members, but not having the field identifier specified
    MemberNotFoundInType(TypeId),
    // IncompatibleLookup(TypeId),
    // Seems like a bit of a jump
    /// Unknown type found
    Unknown(TypeId),
}

/// Collects all members if possible from a given type id
///
/// Return type is empty if the given type cannot carry members
pub fn collect_all_members(
    compiler: &ScriptCompiler,
    mut current_type_id: TypeId,
) -> Vec<MemberId> {
    for _ in 0..chrn_utils::MAX_LOOPS {
        match &compiler.types[current_type_id].ty {
            Type::BuiltinTypeInfo(builtin_type) => todo!(),
            Type::Struct(struct_def) => return struct_def.fields.clone(),
            Type::Enum(enum_def) => return enum_def.variants.clone(),
            // Count members as params or maybe attach a variant?
            Type::Func(func_def) => todo!(),
            Type::Alias(alias_def) => todo!(),
            // Should this?
            Type::TypeDef(type_def) => current_type_id = type_def.type_id,
            Type::Boundaries(type_constraint_flags) => return Vec::new(),
            Type::Deferred(inner_type_id) => current_type_id = *inner_type_id,
            Type::Unknown => return Vec::new(),
        }
    }

    loop_abort!()
}

// Naming has a little collision since member runtime lookup has the same name as this,
// realistically, const lookup.
//
// Not sure about the distinction here yet since member lookup could also mean enum lookup but we'll
// see
// TODO: Lookup patterns
/// Look for the identifier given as a member for the given `TypeId`
pub fn lookup_member(
    compiler: &ScriptCompiler,
    mut current_type_id: TypeId,
    target_name_id: InternedId,
    lookup_pattern: MemberScopeLookupPattern,
) -> MemberLookupResult {
    // Should probably have own `IncompatibleMemberLookup` result
    for _ in 0..chrn_utils::MAX_LOOPS {
        match &compiler.types[current_type_id].ty {
            Type::BuiltinTypeInfo(_) => {
                // Members/Methods do not exist for types yet
                return MemberLookupResult::ImpossibleTypeMemberAccess(current_type_id);
            }
            Type::Struct(struct_def) => {
                for member_id in &struct_def.fields {
                    let field = compiler.get_field(*member_id);
                    if field.name_id == target_name_id {
                        return MemberLookupResult::Found(field.member_id);
                    }
                }

                return MemberLookupResult::MemberNotFoundInType(current_type_id);
            }
            Type::Enum(enum_def) => {
                for member_id in &enum_def.variants {
                    let variant = compiler.get_variant(*member_id);
                    if variant.name_id == target_name_id {
                        return MemberLookupResult::Found(variant.member_id);
                    }
                }

                return MemberLookupResult::MemberNotFoundInType(current_type_id);
            }
            Type::Alias(alias_def) => todo!(),
            Type::Func(func_def) => todo!(),
            // Since typedefs themselves are just fields, we need to treat this as an entry-point to
            // get to the inner type. Given x: State, when the `x` is seen seen it ignores it and
            // skips to the internal type_id field, just like defer does but this is guaranteed to
            // be one layer.
            Type::TypeDef(type_def) => current_type_id = type_def.type_id,
            Type::Boundaries(type_constraint_flags) => todo!(),
            // WARN: DANGEROUS
            Type::Deferred(inner_type_id) => current_type_id = *inner_type_id,

            Type::Unknown => return MemberLookupResult::Unknown(current_type_id),
        }
    }

    loop_abort!()
}
