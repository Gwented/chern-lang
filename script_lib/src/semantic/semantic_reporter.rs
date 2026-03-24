use std::io::IsTerminal;

use common::{
    color,
    fmter::Formattable,
    keywords,
    metadata::ChernMetadata,
    reporter,
    symbols::{Span, TypeId},
};

use crate::{
    algo,
    semantic::error::{Diagnostic, SemanticError},
};

/// Amount of '-' to print for multiple error separation
const TOTAL_SEPARATORS: usize = 60;

#[derive(Debug)]
pub(super) struct SemanticReporter<'a> {
    pub(super) metadata: &'a ChernMetadata,
    pub(super) err_vec: Vec<Diagnostic>,
}

impl SemanticReporter<'_> {
    pub(super) fn new(metadata: &ChernMetadata) -> SemanticReporter<'_> {
        SemanticReporter {
            metadata,
            err_vec: Vec::new(),
        }
    }

    //WARN: Could be better looking
    pub(super) fn report_semantic(&mut self, sem_err: SemanticError) {
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
            reporter::form_err_diag(&self.metadata.src_bytes, &spans, self.metadata.can_color);

        let fmt_msg = reporter::standardize_err(&msg, &ln_data, "");

        let diag = Diagnostic::new(fmt_msg);
        self.err_vec.push(diag);
    }

    /// Draws red arrows under the span given. Option `err_name` represents whether or not a keyword that
    /// could be similar in name should be looked for.
    pub(super) fn report_spanned(&mut self, msg: &str, err_name: Option<&str>, spans: &[Span]) {
        let line_data =
            reporter::form_err_diag(&self.metadata.src_bytes, spans, self.metadata.can_color);

        let help = if let Some(name) = err_name {
            self.try_help(name).unwrap_or_default()
        } else {
            "".to_string()
        };

        // diag_msg?
        let msg = reporter::standardize_err(msg, &line_data, &help);

        let diag = Diagnostic::new(msg.to_owned());

        self.err_vec.push(diag);
    }

    fn try_help(&self, err_name: &str) -> Option<String> {
        let found_kw = algo::fuzzy_match(err_name.as_bytes(), algo::FuzzyMatch::KW)?;

        let msg = format!("Found similar keyword \"{}\"", found_kw);

        let help = reporter::standardize_help(&msg, self.metadata.can_color);

        Some(help)
    }

    pub(super) fn emit_errors(&self) {
        let (red, nc) = color::get_red(self.metadata.can_color);

        let header_err = format!("{red}error{nc}");

        //NOTE: Maybe this should be printed everytime since there could be many prior errors.

        for err in &self.err_vec {
            // Are two syscalls like this constantly like this worst than making it a single string?
            println!("From path => \"{}\"", self.metadata.path.display());
            println!("{header_err}: {}", err.msg);
        }

        eprintln!("\nReported {} error(s)\n", self.err_vec.len());
    }
}
