pub mod inconsistent_comma;
use chrn_utils::source_map::source_diagnostic::SourceDiagnostic;
use compilation::script_compiler::{ScriptCompiler, script_compiler_store::ScriptCompilerStore};

use crate::linter_config::LinterConfig;

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
// Linterable </3
pub trait Lintable {
    fn lint(&self) -> Vec<SourceDiagnostic>;
}

pub fn run_linters(
    compiler: &ScriptCompiler,
    compiler_store: &ScriptCompilerStore,
    lint_cfg: &LinterConfig,
) {
    todo!()
}
