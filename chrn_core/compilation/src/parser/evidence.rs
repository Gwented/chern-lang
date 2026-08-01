// TEST:
// HALLUCINATING IMPLEMENTATION DETAILS
// In the tree for escape info it has to know neutral -> let_env: expected ident && found == kw then
// recommend. BUT, the encoding is more like, if expected ident && found kw, with maybe some || used
// for types of evidence like being in a type declaration or let context. So, hopefully it would be
// an easier let || type decl || etc. level of compression instead of the tree's knowledge being too
// large. The tree. The tree wants a probability model.

use crate::{
    lexer::token::{SpannedToken, TokenKind},
    parser::Branch,
};

//TEST:
/// Data to give in case an error occurs, which is turned into Evidence internally
#[derive(Debug)]
pub(super) struct InitialEvidence {
    pub(super) env: SemanticEnv,
    pub(super) situation: SemanticSituation,
    // May remove this
    /// Granular information instead of flat env
    pub(super) branch: Branch,
}

impl InitialEvidence {
    pub(super) fn new(env: SemanticEnv, situation: SemanticSituation, branch: Branch) -> Self {
        Self {
            env,
            situation,
            branch,
        }
    }
}

#[derive(Debug)]
pub(super) struct Evidence {
    pub(super) env: SemanticEnv,
    pub(super) situation: SemanticSituation,
    pub(super) expected: TokenKind,
    pub(super) found: SpannedToken,
    pub(super) branch: Branch,
}

impl Evidence {
    pub(super) fn new(
        env: SemanticEnv,
        situation: SemanticSituation,
        expected: TokenKind,
        found: SpannedToken,
        branch: Branch,
    ) -> Self {
        Self {
            env,
            situation,
            expected,
            found,
            branch,
        }
    }

    /// Creates evidence while using part of a initial evidence if present.
    pub(super) fn with_initial(
        initial_evidence: InitialEvidence,
        expected: TokenKind,
        found: SpannedToken,
    ) -> Self {
        Self {
            env: initial_evidence.env,
            situation: initial_evidence.situation,
            branch: initial_evidence.branch,
            expected,
            found,
        }
    }
}

#[derive(Debug)]
pub(super) struct EvidenceBuilder {
    env: SemanticEnv,
    situation: SemanticSituation,
    expected: SpannedToken,
    found: SpannedToken,
    // Tokens gone by during propagation to give a rough idea of the slice the evidence needs.
    // Not sure if this would help much
    toks_passed: u32,
}

impl EvidenceBuilder {
    pub(super) fn new(
        env: SemanticEnv,
        situation: SemanticSituation,
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

// Trying to keep this flat..
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(super) enum SemanticEnv {
    SearchingNeutral,
    SearchingSection,
    Bind,
    Let,
    Func,
    Alias,
    Config,
    Import,
    Expr,
    Type,
    ArgList,
    SectNeutral,
    SectVar,
    SectNest,
    SectComplex,
    SectOverride,
}

// Bad name
#[derive(Debug, Clone)]
pub(super) enum SemanticSituation {
    UnexpectedToken,
    /// Expected a keyword
    KeywordBinding,
    /// `expected` is the start delimiter
    MissingStartDelimiter,
    /// Expected an identifier to bind
    IdentBinding,
    ReachedEOF,
    // Duplicate,
    //?
    /// Expected a type to bind to an identifier, or something of that nature
    TypeBinding,
    ValueBinding,
    DirectiveParsing,
    ArgList,
    //???
    /// `expected` is the unclosed delimiter token
    UnclosedDelimiter,
}
