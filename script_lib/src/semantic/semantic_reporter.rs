use chern_core::intern::Intern;
use common::{
    chern_settings::ChernSettings,
    fmter::Formattable,
    reporter::{
        self,
        diagnostic::{Area, Diagnostic},
    },
    span::Span,
};

use crate::{algo, modules::Module, semantic::error::SemanticError};

use super::error::MathError;

#[derive(Debug)]
pub(super) struct SemanticReporter<'a> {
    pub(super) err_vec: Vec<Diagnostic>,
    pub(super) settings: &'a ChernSettings,
    pub(super) interner: &'a Intern,
}

impl<'a> SemanticReporter<'a> {
    pub(super) fn new(settings: &'a ChernSettings, interner: &'a Intern) -> SemanticReporter<'a> {
        SemanticReporter {
            settings,
            interner,
            err_vec: Vec::new(),
        }
    }

    //WARN: Could be better looking
    pub(super) fn report_semantic(&mut self, sem_err: SemanticError, module: &Module) {
        let (msg, spans) = match sem_err {
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
            SemanticError::FuncConstraintMismatch(constraint, type_kind, func_kind, spans) => {
                let msg = format!(
                    "The type `{type_kind}` does not follow constraint `{constraint}` for function \"{func_kind}\""
                );

                (msg, spans)
            }
            SemanticError::ArgMiscount(constraint, func_kind, count, spans) => {
                let msg =
                    format!("Expected `{constraint}` for function \"{func_kind}\", found {count}");

                (msg, spans)
            }
            SemanticError::CircularArg(arg, fmted_ty, spans) => {
                let msg = format!(
                    // Suspicious error message
                    "Cannot give type `{fmted_ty}` the argument \"#{arg}\" due to the circularly referenced type itself not supporting the argument"
                );

                (msg, spans)
            }
            SemanticError::CircularCond(cond, fmted_ty, spans) => {
                let msg = format!(
                    "Cannot give the type `{fmted_ty}` the condition \"{}\" due to the circularly referenced type itself not supporting the condition",
                    cond.to_fmt()
                );
                (msg, spans)
            }
            SemanticError::UnsupportedCond(cond, fmted_ty, spans) => {
                let msg = format!(
                    "The condition \"{}\" is not supported for type `{fmted_ty}`",
                    cond.to_fmt()
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
            },
        };

        let ln_data =
            reporter::form_err_diag(&module.metadata.src_bytes, &spans, self.settings.can_color);

        let fmt_msg = reporter::standardize_err(
            &msg,
            &ln_data,
            "",
            &self.interner.search_path(module.path_id.id as usize),
            self.settings.can_color,
        );

        let diag = Diagnostic::new(fmt_msg, Area::Script);
        self.err_vec.push(diag);
    }

    /// Draws red arrows under the span given. Option `err_name` represents whether or not a keyword that
    /// could be similar in name should be looked for.
    pub(super) fn report_spanned(
        &mut self,
        msg: &str,
        err_name: Option<&str>,
        spans: &[Span],
        module: &Module,
    ) {
        let line_data =
            reporter::form_err_diag(&module.metadata.src_bytes, spans, self.settings.can_color);

        let help = if let Some(name) = err_name {
            self.try_help(name).unwrap_or_default()
        } else {
            "".to_string()
        };

        // diag_msg?
        let msg = reporter::standardize_err(
            msg,
            &line_data,
            &help,
            self.interner.search_path(module.path_id.id as usize),
            self.settings.can_color,
        );

        let diag = Diagnostic::new(msg, Area::Script);

        self.err_vec.push(diag);
    }

    fn try_help(&self, err_name: &str) -> Option<String> {
        let found_kw = algo::fuzzy_match(err_name.as_bytes(), algo::FuzzyMatch::KW)?;

        let msg = format!("Found similar keyword \"{}\"", found_kw);

        let help = reporter::standardize_help(&msg, self.settings.can_color);

        Some(help)
    }
}
