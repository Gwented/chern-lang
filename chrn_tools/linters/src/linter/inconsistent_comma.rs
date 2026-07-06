use chrn_utils::source_map::source_diagnostic::SourceDiagnostic;
use compilation::{lexer::token::SpannedToken, parser::ast::ast_concepts::AstInfo};

use crate::linter::Lintable;

#[derive(Debug)]
pub struct InconsistentCommaLinter<'a> {
    ast_info: &'a AstInfo,
    toks: &'a [SpannedToken],
}

impl InconsistentCommaLinter<'_> {
    pub fn new<'a>(ast_info: &'a AstInfo, toks: &'a [SpannedToken]) -> InconsistentCommaLinter<'a> {
        InconsistentCommaLinter { ast_info, toks }
    }
}

impl Lintable for InconsistentCommaLinter<'_> {
    fn lint(&self) -> Vec<SourceDiagnostic> {
        for thing in self.ast_info.sections() {
            todo!();
        }

        todo!()
    }
}
