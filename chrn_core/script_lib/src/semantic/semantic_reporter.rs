use chrn_utils::intern::Intern;
use common::{
    chrn_settings::ChrnSettings,
    fmter::Formattable,
    reporter::{
        self,
        diagnostic::{Area, Diagnostic},
    },
    span::Span,
};

use crate::{
    algo,
    modules::{Module, ModuleMetadata},
    semantic::error::SemanticError,
};

use super::error::MathError;

#[derive(Debug)]
pub(super) struct SemanticReporter<'a> {
    pub(super) err_vec: Vec<Diagnostic>,
    pub(super) settings: &'a ChrnSettings,
    pub(super) interner: &'a Intern,
}

impl<'a> SemanticReporter<'a> {
    pub(super) fn new(settings: &'a ChrnSettings, interner: &'a Intern) -> SemanticReporter<'a> {
        SemanticReporter {
            settings,
            interner,
            err_vec: Vec::new(),
        }
    }

    //WARN: Could be better looking
    pub(super) fn report_semantic(&mut self, sem_err: SemanticError, metadata: &ModuleMetadata) {
        let (core_msg, spans) = match sem_err {
            SemanticError::UnsupportedArg(arg, type_kind, spans) => {
                let msg = format!(
                    "The argument \"#{}\" is not supported for the type `{}`",
                    arg, type_kind
                );

                (msg, spans)
            }
            SemanticError::VagueArg(inner_arg, spans) => {
                let msg = format!(
                    //FIXME: Still vague
                    "The argument \"#{}\" cannot be used for a `var->` defined variable that holds a \"struct\" or \"enum\"",
                    inner_arg
                );

                (msg, spans)
            }
            SemanticError::FuncConstraintMismatch(constraint, type_kind, spans) => {
                let msg =
                    format!("The type `{type_kind}` does not follow constraint `{constraint}`",);

                (msg, spans)
            }
            SemanticError::ArgCountMismatch(constraint, count, spans) => {
                let msg = format!("Expected {constraint}, found {count}");

                (msg, spans)
            }
            SemanticError::CircularArg(arg, fmted_ty, spans) => {
                let msg = format!(
                    // Suspicious error message
                    "Cannot give type `{fmted_ty}` the argument `#{arg}` due to the circularly referenced type itself not supporting the argument"
                );

                (msg, spans)
            }
            SemanticError::NumericOverflow(id, fmtted_ty, spans) => {
                let overflown_num = self.interner.search(id as usize);
                let msg = format!(
                    "The type `{fmtted_ty}` had an overflow with the value \"{}\" ",
                    overflown_num
                );

                (msg, spans)
            }
            SemanticError::General(msg, spans) => (msg, spans),
            SemanticError::Math(math_error) => match math_error {
                MathError::BinaryOpMismatch(fmtted_lhs, fmtted_rhs, fmtted_op, spans) => {
                    let msg = format!(
                        "The type `{fmtted_lhs}` cannot apply `{fmtted_op}` to type `{fmtted_rhs}`",
                    );

                    (msg, spans)
                }
                MathError::UnaryOpMismatch(fmtted_operand, fmtted_op, spans) => {
                    let msg = format!("Cannot apply `{fmtted_op}` to type `{fmtted_operand}`",);

                    (msg, spans)
                }
                MathError::DivideByZero(_, spans) => {
                    let msg = format!("Cannot divide by zero");

                    (msg, spans);
                    todo!();
                }
            },
            SemanticError::TypeConstraintMismatch(fmtted_constraint, fmtted_err_ty, spans) => {
                // let mut constraints = String::new();
                //
                // for (i, constraint) in fmtted_constraints.iter().enumerate() {
                //     constraints.push_str(&format!("`{constraint}`"));
                //
                //     if i + 2 == fmtted_constraints.len() {
                //         constraints.push_str(" or ");
                //     } else if i + 1 < fmtted_constraints.len() {
                //         constraints.push_str(", ");
                //     }
                // }

                let msg = format!(
                    "`{fmtted_err_ty}` does not satisfy type constraint `{}`",
                    fmtted_constraint
                );

                (msg, spans)
            }
            SemanticError::UndefinedMember(span) => {
                let msg = format!("Cannot infer member access");
                (msg, vec![span])
            }
        };

        let ln_data = reporter::form_err_diag(&metadata.src_bytes, &spans, self.settings.can_color);

        let fmt_msg = reporter::standardize_err(
            &core_msg,
            &ln_data,
            None,
            &self.interner.search_path(metadata.path_id.id as usize),
            self.settings.can_color,
        );

        let path = self.interner.search_path(metadata.path_id.id as usize);

        let diag = Diagnostic::new(
            path,
            core_msg.to_string(),
            Some(common::span::merge_spans(&spans)),
            None,
            fmt_msg,
            Area::Script,
        );
        self.err_vec.push(diag);
    }

    /// Draws red arrows under the span given. Option `err_name` represents whether or not a keyword that
    /// could be similar in name should be looked for.
    pub(super) fn report_spanned(
        &mut self,
        msg: &str,
        err_name: Option<&str>,
        spans: &[Span],
        metadata: &ModuleMetadata,
    ) {
        let ln_data = reporter::form_err_diag(&metadata.src_bytes, spans, self.settings.can_color);

        let help = if let Some(name) = err_name {
            self.try_help(name)
        } else {
            None
        };

        // diag_msg?
        let fmtted_diag = reporter::standardize_err(
            msg,
            &ln_data,
            help.as_ref().map(|s| s.as_str()),
            self.interner.search_path(metadata.path_id.id as usize),
            self.settings.can_color,
        );

        let path = self.interner.search_path(metadata.path_id.id as usize);
        let diag = Diagnostic::new(
            path,
            msg.to_string(),
            Some(common::span::merge_spans(&spans)),
            help,
            fmtted_diag,
            Area::Script,
        );

        self.err_vec.push(diag);
    }

    //TODO: Needs many changes
    fn try_help(&self, err_name: &str) -> Option<String> {
        let found_kw = algo::fuzzy_match(err_name.as_bytes(), algo::FuzzyMatch::KW)?;

        let msg = format!("Found similar keyword \"{}\"", found_kw);

        let help = reporter::standardize_help(&msg, self.settings.can_color);

        Some(help)
    }
}
