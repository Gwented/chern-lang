use chrn_utils::chrn_settings::ChrnSettings;
use chrn_utils::source_map::source_diagnostic::{Reporter, SourceDiagnosticBuilder};
use chrn_utils::{
    intern::Intern,
    source_map::{
        source_diagnostic::{AnnotationKind, DiagnosticLevel, SourceDiagnostic},
        source_region::SourceRegion,
    },
};
use lang::fmter::Formattable;

use crate::semantic::preset_err::PresetErr;

use super::preset_err::{LookupError, MathError};

// These take ownership because `PresetErr::General` will clone otherwise, which isn't expensive.
// Ok maybe this should just be a reference.

/// Convenience function that creates a `SourceDiagnostic` from the given `preset_err` and pushes
/// into `diags`
pub(crate) fn report_preset(
    diags: &mut Vec<SourceDiagnostic>,
    preset_err: PresetErr,
    region: &SourceRegion,
    settings: &ChrnSettings,
    interner: &Intern,
) {
    let diag_builder = create_diag_builder_preset(diags, preset_err, region, settings, interner);
    diags.push(diag_builder.build());
}

// The s is like that on purpose
/// Convenience function that appends `SourceDiagnostic`s from the given `preset_errs` into the
/// given buffer `diags`
pub(crate) fn report_preset_vec(
    diags: &mut Vec<SourceDiagnostic>,
    preset_errs: Vec<PresetErr>,
    region: &SourceRegion,
    settings: &ChrnSettings,
    interner: &Intern,
) {
    for preset in preset_errs {
        let diag_builder = create_diag_builder_preset(diags, preset, region, settings, interner);
        diags.push(diag_builder.build());
    }
}

/// Creates `SourceDiagnostic` with the preset associated with it's `SemanticError`
pub(crate) fn create_diag_builder_preset(
    reporter: &mut Vec<SourceDiagnostic>,
    preset_err: PresetErr,
    region: &SourceRegion,
    settings: &ChrnSettings,
    interner: &Intern,
) -> SourceDiagnosticBuilder {
    match preset_err {
        // Need to know which spans exactly now
        PresetErr::UnsupportedDirective(sp_directive, sym_span) => {
            let directive_constraints = sp_directive
                .inner
                .type_constraints()
                .to_type_constraint_vec();

            let mut constraints_str = String::new();

            for (i, constraint) in directive_constraints.iter().enumerate() {
                constraints_str.push_str(&format!("`{}`", constraint.to_fmt()));

                if i + 1 < directive_constraints.len() {
                    constraints_str.push_str(", ");
                }
            }

            let core_msg = format!(
                "Only types that satisfy {} can use the directive `#{}`",
                constraints_str,
                sp_directive.inner.to_fmt()
            );

            SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, region.path_id)
                .add_annotation(
                    sp_directive.span,
                    AnnotationKind::Secondary,
                    "Constraints required by this directive".to_string().into(),
                )
                .add_annotation(sym_span, AnnotationKind::Primary, None)
        }
        PresetErr::UnknownDirective(sp_interned_id) => {
            let err_name = interner.search(sp_interned_id.inner);
            let core_msg = format!("Unknown directive `#{err_name}`");

            let mut builder =
                SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, region.path_id)
                    .add_annotation(sp_interned_id.span, AnnotationKind::Primary, None);

            //TODO: Search for similar directive
            // Maybe delegate this to uh...um..
            //
            // ignore this
            let similar_vec =
                lang::algo::fuzzy_match(err_name.as_bytes(), lang::algo::FuzzyMatch::Directive);

            //FIX: Every usage of this is very dis-organized
            if !similar_vec.is_empty() {
                let mut help = format!("Found similar directive ");
                for (i, similar) in similar_vec.iter().enumerate() {
                    help.push_str(&format!("`#{similar}`"));
                    if i + 1 != similar_vec.len() {
                        help.push_str(&format!(", "));
                    }
                }

                builder = builder.add_help(help);
            }

            builder
        }
        PresetErr::VagueDirective(sp_directive) => {
            let core_msg = format!(
                //FIXME: Still vague
                "The directive \"#{}\" cannot be used for a `var->` defined variable that holds a `struct` or `enum`",
                sp_directive.inner.to_fmt()
            );

            SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg,  region.path_id)
                    .add_annotation(sp_directive.span, AnnotationKind::Primary, None)
                    .add_note("This is not allowed since it would overlap with any specifics directives given to a defined type from `nest->`".into())
        }
        PresetErr::FuncConstraintMismatch(constraint, type_kind, spans) => {
            todo!();
            // let msg =
            //     format!("The type `{type_kind}` does not satisfy constraint `{constraint}`",);
            //
            // SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg)
            //     .add_annotation(sp_directive.span, AnnotationKind::Primary, None)
            //     .build()
        }
        PresetErr::DirectiveCountMismatch(constraint, count, spans) => {
            todo!();
            // let msg = format!("Expected {constraint}, found {count}");
            //
            // (msg, spans)
        }
        PresetErr::CircularDirective(sp_parent_ty, sp_directive, err_ty_span) => {
            let core_msg = format!(
                // Suspicious error message
                "Cannot give type `{}` the directive `#{}` due to the circularly referenced type itself not supporting the directive",
                sp_parent_ty.inner,
                sp_directive.inner.to_fmt()
            );

            SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, region.path_id)
                .add_annotation(
                    sp_parent_ty.span,
                    AnnotationKind::Secondary,
                    format!("defined here").into(),
                )
                .add_annotation(
                    err_ty_span,
                    AnnotationKind::Secondary,
                    "Recursive".to_string().into(),
                )
                .add_annotation(
                    sp_directive.span,
                    AnnotationKind::Primary,
                    "Conflicting directive here".to_string().into(),
                )
        }
        // Should have the data type's cap shown as well
        PresetErr::NumericOverflow(sp_interned_num, fmtted_ty) => {
            let overflown_num = interner.search(sp_interned_num.inner);
            let core_msg = format!(
                "The type `{fmtted_ty}` had an overflow with the value \"{}\" ",
                overflown_num
            );

            SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, region.path_id)
                .add_annotation(sp_interned_num.span, AnnotationKind::Primary, None)
        }
        PresetErr::General(src_diag) => src_diag,
        PresetErr::Lookup(lookup_err) => match lookup_err {
            LookupError::InvalidTypeMemberAccess(sp_fmtted_ty) => {
                let core_msg = format!(
                    "Type `{}` does not have the ability to hold members",
                    sp_fmtted_ty.inner,
                );

                SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, region.path_id)
                    .add_annotation(sp_fmtted_ty.span, AnnotationKind::Primary, None)
            }
            LookupError::MemberNotFound(sp_interned_ty, member_ident) => {
                let ty_name = interner.search(sp_interned_ty.inner);
                let member_name = interner.search(member_ident);
                let core_msg = format!(
                    "No member with the identifier \"{member_name}\" was found in type `{ty_name}`"
                );

                SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, region.path_id)
                    .add_annotation(sp_interned_ty.span, AnnotationKind::Primary, None)
            }
            LookupError::InvalidSymbolMemberAccess(sp_sym) => {
                let core_msg = format!("Symbol `{}` cannot use member access", sp_sym.inner);
                SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, region.path_id)
                    .add_annotation(sp_sym.span, AnnotationKind::Primary, None)
            }
        },
        PresetErr::Math(math_err) => match math_err {
            MathError::BinaryOpMismatch(sp_fmtted_lhs, sp_fmtted_rhs, fmtted_op) => {
                let core_msg = format!(
                    "The type `{}` cannot apply `{fmtted_op}` to type `{}`",
                    sp_fmtted_lhs.inner, sp_fmtted_rhs.inner,
                );

                SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, region.path_id)
                    .add_annotation(sp_fmtted_lhs.span, AnnotationKind::Primary, None)
                    .add_annotation(sp_fmtted_rhs.span, AnnotationKind::Primary, None)
            }
            MathError::UnaryOpMismatch(fmtted_operand, fmtted_op) => {
                let core_msg = format!(
                    "Cannot apply `{}` to type `{}`",
                    fmtted_op, fmtted_operand.inner
                );

                SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, region.path_id)
                    .add_annotation(fmtted_operand.span, AnnotationKind::Primary, None)
            }
            MathError::DivideByZero(_, spans) => {
                todo!("Need to fix inner of evaluator functions");
                let msg = format!("Cannot divide by zero");

                (msg, spans);
                todo!();
            }
        },
        // Maybe a type version too?
        PresetErr::TypeConstraintMismatch(given_constraints, fmtted_found_ty, spans) => {
            let given_vec = given_constraints.to_type_constraint_vec();
            let mut given_str = String::new();

            for (i, constraint) in given_vec.iter().enumerate() {
                given_str.push_str(&format!("`{}`", constraint.to_fmt()));

                if i + 1 < given_vec.len() {
                    given_str.push_str(", ");
                }
            }

            let msg = format!(
                "`{}` does not satisfy constraint {}",
                fmtted_found_ty, given_str
            );

            todo!();
        }
        //TODO: Not done
        PresetErr::UndefinedMember(span) => {
            let core_msg = format!("Cannot infer member access");
            SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, region.path_id)
                .add_annotation(
                    span,
                    AnnotationKind::Primary,
                    "Nothing is inferred to match this member"
                        .to_string()
                        .into(),
                )
        }
        PresetErr::TypeConstraintBoundConflict(current_inferred, conflicting_inferred, spans) => {
            //FIXME: Fear inducing message.
            let current_bounds = current_inferred.to_type_constraint_vec();
            let conflicting_bounds = conflicting_inferred.to_type_constraint_vec();

            let mut current_msg = String::new();

            if current_bounds.len() > 1 {
                current_msg.push_str("constraints ");
            } else {
                current_msg.push_str("constraint ");
            }

            for (i, bound) in current_bounds.iter().enumerate() {
                current_msg.push_str(&format!("`{}`", bound.to_fmt()));
                if i + 1 < current_bounds.len() {
                    current_msg.push_str(" + ");
                }
            }

            let mut conflicting_msg = String::new();
            for (i, bound) in conflicting_bounds.iter().enumerate() {
                conflicting_msg.push_str(&format!("`{}`", bound.to_fmt()));
                if i + 1 < conflicting_bounds.len() {
                    conflicting_msg.push_str(" + ");
                }
            }

            let msg = format!(
                "Inferred {current_msg} conflicts with another expression's constraints of {conflicting_msg}"
            );

            todo!()
        }
    }
}

//TODO: Needs many changes
// fn try_help(&self, err_name: &str) -> Option<String> {
//     let found_kw = algo::fuzzy_match(err_name.as_bytes(), algo::FuzzyMatch::KW)?;
//
//     let msg = format!("Found similar keyword \"{}\"", found_kw);
//
//     let help = reporter::standardize_help(&msg,  settings.can_color);
//
//     Some(help)
// }
