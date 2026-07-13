use chrn_utils::{
    id_types::{InternedId, SymbolId, TypeId},
    loop_abort,
};
use lang::{
    config_schemas::{self, ConfigSchema, ConfigSchemaKind, OptionSchemaConstraint},
    types::boundaries::TypeBoundaryFlags,
    values::Value,
};

use crate::{script_compiler::ScriptCompiler, semantic::hir::hir_concepts::Type};

/// Result type for schema lookups. This exists due to the fact that there is no `Ok` or `Err`
/// inherit concept behind whether or not something was found.
#[derive(Debug, Clone)]
pub enum SchemaResult {
    /// The schema was...valid
    Valid,
    // How about "bounds"?
    /// Contains the index of the value that failed from the slice of values given
    BoundaryMismatch {
        err_idx: usize,
        required_boundaries: TypeBoundaryFlags,
        err_boundaries: TypeBoundaryFlags,
    },
    // Do functions have boundaries based off return type?
    /// Option has boundaries but the value being checked is invalid in regards to having boundaries
    /// in the first place.
    ///
    /// Contains the index of the value that failed from the slice of values given
    NoBoundariesInValue {
        err_idx: usize,
        required_boundaries: TypeBoundaryFlags,
    },
    /// An example of this is an option like `default_val` being used for something like an
    /// enum variant "State { state }" where state doesn't actually have a type attached.
    /// This would mean that `state` fundamentally has no boundaries so any option that may
    /// require the config it's apart of to fulfill some form of boundary would be impossible,
    /// since `state` cannot support boundaries.
    CannotSupportBoundaries,
    // A little too specific right now. Likely need to collapse this just like the arg constraints
    // were where they have their to_string so that all of these cases are RO#$I@#O
    /// Expected each value to be of the same as the config having it's properties defined.
    SameTypeAsUserMismatch {
        err_idx: usize,
        err_boundaries_opt: Option<TypeBoundaryFlags>,
        user_boundaries: TypeBoundaryFlags,
    },
    /// Identifier given doesn't exist for particular schema kind.
    UnknownOptionName,
}

//TEST: This language construct is still confusing to find the best version of.
/// Uses `TypeId` to search for if there is a schema available to match contents against
pub fn get_schema_from_type_id(
    compiler: &ScriptCompiler,
    mut current_type_id: TypeId,
) -> Option<&'static ConfigSchema> {
    for _ in 0..chrn_utils::MAX_LOOPS {
        match &compiler.types[current_type_id].ty {
            Type::Struct(_) => {
                return Some(config_schemas::get_cfg_schema(ConfigSchemaKind::Struct));
            }
            Type::Enum(_) => {
                return Some(config_schemas::get_cfg_schema(ConfigSchemaKind::Enum));
            }
            Type::Deferred(type_id) => current_type_id = *type_id,
            Type::Alias(_)
            | Type::Boundaries(_)
            | Type::BuiltinTypeInfo(_)
            | Type::TypeDef(_)
            | Type::Func(_)
            | Type::Unknown => return None,
        }
    }
    loop_abort!()
}

/// GIVE ME THE RIGHT SCHEMA question_mark
pub fn validate_opt(
    schema: &ConfigSchema,
    // User as in the config member/root's boundaries not an actual user
    //
    // This is more like, cfg_ty_boundaries
    user_boundaries_opt: Option<TypeBoundaryFlags>,
    // Given like this so it doesn't matter if it's a root or member option
    opt_name_id: InternedId,
    opt_values: &[Value],
) -> SchemaResult {
    // This messes with past conventions of opt meaning Option<T> not option option Options..
    let Some(schema_opt) = schema.get_opt(opt_name_id) else {
        return SchemaResult::UnknownOptionName;
    };

    // Specific checks like, if this is a field, can this use this option, are answered by the
    // schema itself, assuming the schema given is right.
    match &schema_opt.boundaries {
        Some(required_opt_constraints) => {
            match required_opt_constraints {
                // [1]
                OptionSchemaConstraint::Boundaries(required_boundaries) => {
                    let required_boundaries = *required_boundaries;
                    // Ok
                    for (i, val) in opt_values.iter().enumerate() {
                        // Since valid or not is not descriptive, we should probably have this
                        // as spanned as the other non-assuming result types
                        let Some(current_boundaries) = val.kind().boundaries() else {
                            return SchemaResult::NoBoundariesInValue {
                                err_idx: i,
                                required_boundaries,
                            };
                        };

                        if !current_boundaries.overlaps(required_boundaries) {
                            return SchemaResult::BoundaryMismatch {
                                err_idx: i,
                                required_boundaries,
                                err_boundaries: current_boundaries,
                            };
                        }
                    }
                }
                // [2]
                OptionSchemaConstraint::SameTypeAsConfig => {
                    for (i, val) in opt_values.iter().enumerate() {
                        // dbg!(val.kind().boundaries(), user_boundaries_opt);

                        // Is matching all cases because this will have more cases, eventually.
                        let current_boundaries_opt = val.kind().boundaries();
                        match (current_boundaries_opt, user_boundaries_opt) {
                            (Some(current), Some(user)) => {
                                if !current.overlaps(user) {
                                    return SchemaResult::SameTypeAsUserMismatch {
                                        err_idx: i,
                                        err_boundaries_opt: current_boundaries_opt,
                                        user_boundaries: user,
                                    };
                                }
                            }
                            // None from user boundaries could mean something like an enum with no
                            // type attached was used, where the user boundaries now reflect a None
                            // boundary and is trying to be matched against say, numbers. Should
                            // this get it's own variant since it's more so, this thing CANNOT
                            // fulfill any boundaries and you gave it a value with boundaries.
                            (Some(_), None) => {
                                return SchemaResult::CannotSupportBoundaries;
                            }
                            // None from current means it blatantly isn't capable of fulfilling any
                            // user boundaries, I believe.
                            (None, Some(user)) => {
                                return SchemaResult::SameTypeAsUserMismatch {
                                    err_idx: i,
                                    err_boundaries_opt: current_boundaries_opt,
                                    user_boundaries: user,
                                };
                            }
                            // This isn't BAD it's just not something that exists right now since
                            // the schema constraint itself wouldn't exist if it didn't have boundaries
                            (None, None) => unreachable!(),
                        }
                    }
                }
            }

            SchemaResult::Valid
        }
        // Nothing to actually check against value-wise since there are no boundaries
        None => SchemaResult::Valid,
    }
}
