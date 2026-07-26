// TEST:
// HALLUCINATING IMPLEMENTATION DETAILS
// In the tree for escape info it has to know neutral -> let_env: expected ident && found == kw then
// recommend. BUT, the encoding is more like, if expected ident && found kw, with maybe some || used
// for types of evidence like being in a type declaration or let context. So, hopefully it would be
// an easier let || type decl || etc. level of compression instead of the tree's knowledge being too
// large. The tree. The tree wants a probability model.

use crate::lexer::token::SpannedToken;

#[derive(Debug)]
pub(super) struct Evidence {
    env: EvidenceEnv,
    situation: Situation,
    expected: SpannedToken,
    found: SpannedToken,
}

impl Evidence {
    pub(super) fn new(
        env: EvidenceEnv,
        situation: Situation,
        expected: SpannedToken,
        found: SpannedToken,
    ) -> Self {
        Self {
            env,
            situation,
            expected,
            found,
        }
    }
}

#[derive(Debug)]
pub(super) struct EvidenceBuilder {
    env: EvidenceEnv,
    situation: Situation,
    expected: SpannedToken,
    found: SpannedToken,
    // Tokens gone by during propagation to give a rough idea of the slice the evidence needs.
    // Not sure if this would help much
    toks_passed: u32,
}

impl EvidenceBuilder {
    pub(super) fn new(
        env: EvidenceEnv,
        situation: Situation,
        expected: SpannedToken,
        found: SpannedToken,
        toks_passed: u32,
    ) -> Self {
        Self {
            env,
            situation,
            expected,
            found,
            toks_passed,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(super) enum EvidenceEnv {
    Searching,
    Bind,
    Let,
    Func,
    Alias,
    Import,
    Expr,
    ArgList,
    SectNeutral,
    SectVar,
    SectNest,
    SectComplex,
    SectOverride,
}

// Bad name
#[derive(Debug, Clone)]
pub(super) enum Situation {
    UnexpectedToken,
    ArgList,
    //???
    /// The starting delimiter
    UnclosedDelimiter {
        delimiter: SpannedToken,
    },
}
