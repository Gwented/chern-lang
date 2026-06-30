pub mod inconsistent_comma;
use chrn_utils::source_map::source_diagnostic::SourceDiagnostic;

use crate::{linter::inconsistent_comma::InconsistentCommaLinter, linter_config::LinterConfig};

pub(crate) const INCONSISTENT_COMMA: u16 = 0;

pub(crate) static LINT_ARRAY: [u16; 1] = [INCONSISTENT_COMMA];

#[derive(Debug, Clone, Copy)]
pub(crate) struct LintId {
    id: u16,
}

// Not sure if this is needed since these should all be warns, probably.
#[derive(Debug, Clone, Copy)]
pub struct LintInfo {
    lint_id: LintId,
}

//TEST:
pub trait Linterable {
    fn lint(&self) -> Vec<SourceDiagnostic>;
}

pub fn run_linters(lint_cfg: &LinterConfig) {
    todo!()
}
