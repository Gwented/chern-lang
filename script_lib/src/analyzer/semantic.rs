use std::io::IsTerminal;

use common::{
    keywords,
    metadata::FileMetadata,
    reporter,
    symbols::{Span, TypedId},
};

use crate::{
    algo,
    analyzer::{SemanticError, error::Diagnostic},
};

/// Amount of '-' to print for multiple error separation
const TOTAL_SEPARATORS: usize = 60;

#[derive(Debug)]
pub(super) struct SemanticReporter<'a> {
    pub(super) metadata: &'a FileMetadata,
    pub(super) err_vec: Vec<Diagnostic>,
}

impl SemanticReporter<'_> {
    pub(super) fn new(metadata: &FileMetadata) -> SemanticReporter<'_> {
        SemanticReporter {
            metadata,
            err_vec: Vec::new(),
        }
    }

    //WARN: Could be better looking
    pub(super) fn report_semantic(&mut self, sem_err: SemanticError) {
        let (msg, line_data) = match sem_err {
            SemanticError::UnsupportedArg(spanned_arg, builtin_type_kind) => {
                let msg = format!(
                    "The argument \"#{}\" is not supported for the type \"{}\"",
                    spanned_arg.inner_arg, builtin_type_kind
                );

                let line_data = reporter::form_err_diag(
                    &self.metadata.src_bytes,
                    &spanned_arg.span,
                    self.metadata.can_color,
                );

                (msg, line_data)
            }
            SemanticError::VagueArg(inner_arg, span) => {
                let msg = format!(
                    //FIXME: Still vague
                    "The argument \"#{}\" cannot be used for `struct` or `enum` types when another struct is a field variable",
                    inner_arg
                );

                let line_data = reporter::form_err_diag(
                    &self.metadata.src_bytes,
                    &span,
                    self.metadata.can_color,
                );
                (msg, line_data)
            }
        };

        let fmt_msg = reporter::standardize_err(&msg, &line_data, "");

        let diag = Diagnostic::new(fmt_msg);
        self.err_vec.push(diag);

        // let msg = reporter::standardize_err(base_msg, line_data, help);
    }

    /// Draws red arrows under the span given. Option `err_name` represents whether or not a keyword that
    /// could be similar in name should be looked for.
    pub(super) fn report_spanned(&mut self, msg: &str, err_name: Option<&str>, span: &Span) {
        let line_data =
            reporter::form_err_diag(&self.metadata.src_bytes, span, self.metadata.can_color);

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
        let header_err = if self.metadata.can_color {
            format!("{}error{}", reporter::RED, reporter::NC)
        } else {
            format!("error")
        };

        //NOTE: Maybe this should be printed everytime since there could be many prior errors.

        for err in &self.err_vec {
            // Are two syscalls like this constantly like this worst than making it a single string?
            println!("From path => {}", self.metadata.path.display());
            println!("{header_err}: {}", err.msg);
        }

        eprintln!("\nReported {} error(s)\n", self.err_vec.len());
    }
}
