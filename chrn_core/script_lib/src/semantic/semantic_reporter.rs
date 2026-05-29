use chrn_utils::{
    chrn_settings::ChrnSettings,
    fmter::Formattable,
    intern::Intern,
    source_map::{
        source_diagnostic::{AnnotationKind, DiagnosticLevel, SourceDiagnostic},
        source_region_data::SourceRegion,
    },
};

use crate::semantic::error::SemanticError;

use super::error::MathError;

#[derive(Debug)]
pub(super) struct SemanticReporter<'a> {
    pub(super) err_vec: Vec<SourceDiagnostic>,
    pub(super) region: &'a SourceRegion,
    pub(super) settings: &'a ChrnSettings,
    pub(super) interner: &'a Intern,
}

impl<'a> SemanticReporter<'a> {
    pub(super) fn new(
        settings: &'a ChrnSettings,
        current_region: &'a SourceRegion,
        interner: &'a Intern,
    ) -> SemanticReporter<'a> {
        SemanticReporter {
            settings,
            region: current_region,
            interner,
            err_vec: Vec::new(),
        }
    }

    //WARN: Could be better looking
    pub(super) fn report_semantic(&mut self, sem_err: SemanticError) {
        let src_diag = match sem_err {
            // Need to know which spans exactly now
            SemanticError::UnsupportedArg(sp_arg, sym_span) => {
                let arg_constraints = sp_arg.arg.type_constraints().to_type_constraint_vec();

                let mut constraints_str = String::new();

                for (i, constraint) in arg_constraints.iter().enumerate() {
                    constraints_str.push_str(&format!("`{}`", constraint.to_fmt()));

                    if i + 1 < arg_constraints.len() {
                        constraints_str.push_str(", ");
                    }
                }

                let core_msg = format!(
                    "Only types that satisfy {} can use the argument `#{}`",
                    constraints_str, sp_arg.arg
                );

                SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, self.region.path_id)
                    .add_annotation(
                        sp_arg.span,
                        AnnotationKind::Secondary,
                        "Constraints required by this argument".to_string().into(),
                    )
                    .add_annotation(sym_span, AnnotationKind::Primary, None)
                    .build()
            }
            SemanticError::VagueArg(sp_arg) => {
                let core_msg = format!(
                    //FIXME: Still vague
                    "The argument \"#{}\" cannot be used for a `var->` defined variable that holds a \"struct\" or \"enum\"",
                    sp_arg.arg
                );

                SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, self.region.path_id)
                    .add_annotation(sp_arg.span, AnnotationKind::Primary, None)
                    .add_note("This is not allowed since it would overlap with any specifics arguments given to a defined type from `nest->`".into())
                    .build()
            }
            SemanticError::FuncConstraintMismatch(constraint, type_kind, spans) => {
                todo!();
                // let msg =
                //     format!("The type `{type_kind}` does not satisfy constraint `{constraint}`",);
                //
                // SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg)
                //     .add_annotation(sp_arg.span, AnnotationKind::Primary, None)
                //     .build()
            }
            SemanticError::ArgCountMismatch(constraint, count, spans) => {
                todo!();
                // let msg = format!("Expected {constraint}, found {count}");
                //
                // (msg, spans)
            }
            SemanticError::CircularArg(parent_span, sp_arg, sp_fmtted_ty) => {
                let core_msg = format!(
                    // Suspicious error message
                    "Cannot give type `{}` the argument `#{}` due to the circularly referenced type itself not supporting the argument",
                    sp_fmtted_ty.fmtted, sp_arg.arg
                );

                SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, self.region.path_id)
                    .add_annotation(
                        parent_span,
                        AnnotationKind::Secondary,
                        format!("{} defined here", sp_fmtted_ty.fmtted).into(),
                    )
                    .add_annotation(sp_fmtted_ty.span, AnnotationKind::Primary, None)
                    .add_annotation(
                        sp_arg.span,
                        AnnotationKind::Secondary,
                        "Conflicting argument".to_string().into(),
                    )
                    .build()
            }
            // Should have the data type's cap shown as well
            SemanticError::NumericOverflow(sp_interned_num, fmtted_ty) => {
                let overflown_num = self.interner.search(sp_interned_num.interned_id);
                let core_msg = format!(
                    "The type `{fmtted_ty}` had an overflow with the value \"{}\" ",
                    overflown_num
                );

                SourceDiagnostic::basic(
                    DiagnosticLevel::Error,
                    core_msg,
                    self.region.path_id,
                    sp_interned_num.span,
                )
            }
            SemanticError::General(src_diag) => src_diag,
            SemanticError::Math(math_error) => match math_error {
                MathError::BinaryOpMismatch(sp_fmtted_lhs, sp_fmtted_rhs, fmtted_op) => {
                    let core_msg = format!(
                        "The type `{}` cannot apply `{fmtted_op}` to type `{}`",
                        sp_fmtted_lhs.fmtted, sp_fmtted_rhs.fmtted,
                    );

                    SourceDiagnostic::basic_multiple(
                        DiagnosticLevel::Error,
                        core_msg,
                        self.region.path_id,
                        &[sp_fmtted_lhs.span, sp_fmtted_rhs.span],
                    )
                }
                MathError::UnaryOpMismatch(fmtted_operand, fmtted_op) => {
                    let core_msg = format!(
                        "Cannot apply `{}` to type `{}`",
                        fmtted_op, fmtted_operand.fmtted
                    );

                    SourceDiagnostic::basic(
                        DiagnosticLevel::Error,
                        core_msg,
                        self.region.path_id,
                        fmtted_operand.span,
                    )
                }
                MathError::DivideByZero(_, spans) => {
                    todo!("Need to fix inner of evaluator functions");
                    let msg = format!("Cannot divide by zero");

                    (msg, spans);
                    todo!();
                }
            },
            // Maybe a type version too?
            SemanticError::TypeConstraintMismatch(given_constraints, fmtted_found_ty, spans) => {
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
            SemanticError::UndefinedMember(span) => {
                let core_msg = format!("Cannot infer member access");
                SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, self.region.path_id)
                    .add_annotation(
                        span,
                        AnnotationKind::Primary,
                        "Nothing is inferred to match this member"
                            .to_string()
                            .into(),
                    )
                    .build()
            }
            SemanticError::TypeConstraintBoundConflict(
                current_inferred,
                conflicting_inferred,
                spans,
            ) => {
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
        };

        self.err_vec.push(src_diag);
    }

    //TODO: Needs many changes
    // fn try_help(&self, err_name: &str) -> Option<String> {
    //     let found_kw = algo::fuzzy_match(err_name.as_bytes(), algo::FuzzyMatch::KW)?;
    //
    //     let msg = format!("Found similar keyword \"{}\"", found_kw);
    //
    //     let help = reporter::standardize_help(&msg, self.settings.can_color);
    //
    //     Some(help)
    // }
}
