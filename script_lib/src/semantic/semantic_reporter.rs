use common::{
    fmter::Formattable,
    intern::Intern,
    metadata::{ChernSettings, ModuleMetadata},
    reporter::{
        self,
        diagnostic::{Area, Diagnostic},
    },
    symbols::{Span, TypeId},
};

use crate::{algo, modules::Module, semantic::error::SemanticError};

#[derive(Debug)]
pub(super) struct SemanticReporter<'a> {
    pub(super) err_vec: Vec<Diagnostic>,
    pub(super) settings: &'a ChernSettings,
}

impl<'a> SemanticReporter<'a> {
    pub(super) fn new(settings: &'a ChernSettings) -> SemanticReporter<'a> {
        SemanticReporter {
            settings,
            err_vec: Vec::new(),
        }
    }

    //WARN: Could be better looking
    pub(super) fn report_semantic(
        &mut self,
        sem_err: SemanticError,
        mod_metadata: &ModuleMetadata,
    ) {
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
            SemanticError::ConstraintMismatch(constraint, type_kind, func_kind, spans) => {
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
            // Weird merge needs to happen but both just need to be Formatted Formatted
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
        };

        let ln_data =
            reporter::form_err_diag(&mod_metadata.src_bytes, &spans, self.settings.can_color);

        let fmt_msg = reporter::standardize_err(
            &msg,
            &ln_data,
            "",
            &mod_metadata.path,
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
        mod_metadata: &ModuleMetadata,
    ) {
        let line_data =
            reporter::form_err_diag(&mod_metadata.src_bytes, spans, self.settings.can_color);

        let help = if let Some(name) = err_name {
            self.try_help(name, &mod_metadata).unwrap_or_default()
        } else {
            "".to_string()
        };

        // diag_msg?
        let msg = reporter::standardize_err(
            msg,
            &line_data,
            &help,
            &mod_metadata.path,
            self.settings.can_color,
        );

        let diag = Diagnostic::new(msg, Area::Script);

        self.err_vec.push(diag);
    }

    fn try_help(&self, err_name: &str, mod_metadata: &ModuleMetadata) -> Option<String> {
        let found_kw = algo::fuzzy_match(err_name.as_bytes(), algo::FuzzyMatch::KW)?;

        let msg = format!("Found similar keyword \"{}\"", found_kw);

        let help = reporter::standardize_help(&msg, self.settings.can_color);

        Some(help)
    }
}
