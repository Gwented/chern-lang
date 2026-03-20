use common::{
    builtins::BuiltinTypeKind,
    symbols::{InnerArgs, Span, SpannedInnerArgs, TypedId},
};

use crate::semantic::representation::{ArgConstraint, FuncKind};

#[derive(Debug)]
// Lifetimes
pub(super) enum SemanticError {
    // constraint, found type, what function type, span
    ConstraintMismatch(ArgConstraint, BuiltinTypeKind, FuncKind, Span),
    // constraint, function type, incorrect params found, span
    ArgMiscount(ArgConstraint, FuncKind, u8, Span),
    // argument failed at, found type
    UnsupportedArg(SpannedInnerArgs, BuiltinTypeKind),
    // Interesting name
    VagueArg(InnerArgs, Span),
}

#[derive(Debug)]
pub(super) struct Diagnostic {
    //FIX:
    pub(super) msg: String,
    // Maybe help
    // pub(crate) help: Option<String>
}

impl Diagnostic {
    // TODO: May change both to &str
    pub(super) fn new(msg: String) -> Diagnostic {
        Diagnostic { msg }
    }
}
