use std::fmt::Display;

use chrn_utils::{fmter::Formattable, id_types::TypeId, source_map::source_span::SourceSpan};
use lang::types::type_constraints::TypeConstraintFlags;

use crate::{
    script_compiler::ScriptCompiler,
    semantic::{error::SemanticError, hir::Type},
};

//TEST:
/// Checks given type against the constraints given
pub(super) fn check_type_constraint(
    compiler: &ScriptCompiler,
    type_id: TypeId,
    ty_span: SourceSpan,
    cond_span: SourceSpan,
    visited: &mut Vec<TypeId>,
    given_constraints: TypeConstraintFlags,
) -> Result<(), SemanticError> {
    let ty = &compiler.types[type_id.id as usize].ty;
    match ty {
        Type::Struct(struct_def) => {
            // let symbol = &script_compiler.symbols[struct_def.sym_id.id as usize];
            // let ast_id = symbol.ast_id.expect("Core should not be resolved");
            // let abs_struct = &self.ast_info.get_struct(ast_id);
            visited.push(type_id);

            // No cross module reporting so all messages are shallow in spanning
            for (i, member_id) in struct_def.fields.iter().enumerate() {
                let field = &compiler.get_field(*member_id);
                // Not sure if this incurs any errors this time
                if visited.contains(&field.type_id) {
                    // if spanned_arg.arg.has_restrictions() {
                    //     let name = self.interner.search(symbol.name_id.id as usize);
                    //
                    //     let msg = format!(
                    //         "The type `{name}` cannot have `#{}` applied due to recursively relying on itself satisfying the argument",
                    //         spanned_arg.arg
                    //     );
                    //
                    //     return Err(SemanticError::General(
                    //         msg,
                    //         vec![spanned_arg.span, active_span],
                    //     ));
                    // }

                    continue;
                }

                visited.push(field.type_id);

                check_type_constraint(
                    compiler,
                    field.type_id,
                    ty_span,
                    cond_span,
                    visited,
                    given_constraints,
                )?;
            }

            Ok(())
        }
        Type::Enum(enum_def) => {
            visited.push(type_id);

            for member_id in &enum_def.variants {
                let variant = compiler.get_variant(*member_id);
                if let Some(inner_id) = variant.type_id {
                    visited.push(inner_id);

                    // Checking if one of it's variants are self referencing, or if the type we
                    // just came from, possibly a tuple, is referring to itself from a
                    // different context.
                    if visited.contains(&inner_id) {
                        continue;
                    }

                    check_type_constraint(
                        compiler,
                        inner_id,
                        ty_span,
                        cond_span,
                        visited,
                        given_constraints,
                    )?;
                }
            }

            Ok(())
        }
        //WARN: Suspicious
        Type::Func(_) => todo!(),
        // Not quite sure about this
        Type::Alias(alias_def) => {
            // Misleading error message
            if given_constraints.contains(alias_def.ty_constraints) {
                return Err(SemanticError::TypeConstraintBoundConflict(
                    given_constraints,
                    alias_def.ty_constraints,
                    vec![ty_span, cond_span],
                ));
            }
            panic!("Is does not contain given");

            Ok(())
        }
        Type::BuiltinType(builtin_ty) => {
            //TODO: Allow optionally to choose if a condition should be shallow or not

            let constraints = builtin_ty.kind().type_constraints();

            if !given_constraints.contains(constraints) {
                return Err(SemanticError::TypeConstraintMismatch(
                    given_constraints,
                    builtin_ty.kind().to_fmt(),
                    vec![ty_span, cond_span],
                ));
            }

            Ok(())
        }
        Type::Constrained(constraints) => {
            if !given_constraints.contains(*constraints) {
                return Err(SemanticError::TypeConstraintBoundConflict(
                    given_constraints,
                    *constraints,
                    vec![ty_span, cond_span],
                ));
            }

            Ok(())
        }
        // Type::TypeDef(type_def) => {},
        // Type::Unknown => todo!(),
        _ => unreachable!("Unreachable I think?"),
    }
}

pub fn get_type_constraints(
    compiler: &ScriptCompiler,
    type_id: TypeId,
    ty_span: SourceSpan,
    is_rec: bool,
) -> Option<TypeConstraintFlags> {
    let ty = &compiler.types[type_id.id as usize].ty;
    match ty {
        Type::BuiltinType(builtin_ty) => Some(builtin_ty.kind().type_constraints()),
        // Is it?
        Type::Struct(_) | Type::Enum(_) if !is_rec => None,
        // Have to check if every field in a given struct or enum is aligned under a constraint
        Type::Struct(struct_def) => todo!(),
        Type::Enum(enum_def) => todo!(),
        Type::Func(func_def) => Some(func_def.type_constraints),
        Type::Alias(alias_def) => todo!(),
        Type::Constrained(ty_constraint_flags) => Some(*ty_constraint_flags),
        // Wait should this?
        Type::TypeDef(type_def) => {
            get_type_constraints(compiler, type_def.type_id, ty_span, is_rec)
        }
        Type::Deferred(deferred_ty_id) => {
            get_type_constraints(compiler, *deferred_ty_id, ty_span, is_rec)
        }
        Type::Unknown => None,
    }
}

// Nat
// Real
// Complex
// Prime
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgConstraint {
    ArgCount(u32),
    // Ordering(OrderingType),
    DynType,
    MatchingArgumentTypes,
    /// Must be the same type as the type the condition is made for
    Numeric,
    Integer,
    Float,
    CharacterMappable,
    Bool,
    Str,
    Comparable,
    // Ok?
    SameTypeAsSelf,
    // Collection,
    // Suspicious
    Variadic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderingType {
    Greater,
    GreaterOrEq,
    LessThan,
    LessOrEq,
    Eq,
}
//
// impl ArgConstraint {
//     // TODO: Composable constraints for aliases
//     /// Takes in a function kind that is built in and returns it's constraints
//     pub fn from_builtin(kind: FuncKind) -> Vec<ArgConstraint> {
//         match kind {
//             FuncKind::IsEmpty => vec![ArgConstraint::ArgCount(0), ArgConstraint::Str],
//             FuncKind::StartsW => {
//                 // Maybe if we got something like 0x1FF it could StartsW(0x1FF)?
//                 vec![ArgConstraint::ArgCount(1), ArgConstraint::DynType]
//             }
//             FuncKind::EndsW => {
//                 vec![ArgConstraint::ArgCount(1), ArgConstraint::DynType]
//             }
//             FuncKind::Contains => {
//                 vec![ArgConstraint::ArgCount(1), ArgConstraint::DynType]
//             }
//             FuncKind::Range => {
//                 vec![
//                     ArgConstraint::ArgCount(2),
//                     ArgConstraint::Numeric,
//                     ArgConstraint::MatchingArgumentTypes,
//                 ]
//             }
//             FuncKind::Equals => {
//                 vec![ArgConstraint::Variadic]
//             }
//             FuncKind::IsWhitespace => {
//                 vec![ArgConstraint::ArgCount(0), ArgConstraint::CharacterMappable]
//             }
//         }
//     }
// }

impl Display for ArgConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArgConstraint::DynType => write!(f, "DynamicType"),
            ArgConstraint::MatchingArgumentTypes => write!(f, "MatchingArgumentType"),
            ArgConstraint::Numeric => write!(f, "Numeric"),
            ArgConstraint::Integer => write!(f, "Integer"),
            ArgConstraint::Float => write!(f, "Float"),
            ArgConstraint::Str => write!(f, "str"),
            ArgConstraint::ArgCount(count) => {
                if *count == 0 || *count > 1 {
                    write!(f, "{count} arguments")
                } else {
                    write!(f, "{count} argument")
                }
            }
            ArgConstraint::CharacterMappable => write!(f, "CharacterMappable"),
            ArgConstraint::Variadic => write!(f, "variadic"),
            ArgConstraint::Bool => write!(f, "bool"),
            ArgConstraint::Comparable => write!(f, "Comparable"),
            ArgConstraint::SameTypeAsSelf => write!(f, "Same type as self"),
        }
    }
}

// Can't really use AstId since it's not an ExprId and it would pretty much be a guess as to what
// typeexpr it came from
