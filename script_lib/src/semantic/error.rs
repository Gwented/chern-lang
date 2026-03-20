use common::{
    builtins::BuiltinTypeKind,
    symbols::{InnerArgs, Span, SpannedInnerArgs, TypedId},
};

use crate::semantic::representation::{ArgConstraint, FuncKind};

#[derive(Debug)]
// Lifetimes
pub(super) enum SemanticError {
    // Interesting names
    // expected, found, what function type
    TypeMismatch(ArgConstraint, BuiltinTypeKind, FuncKind, Span),
    VagueArg(InnerArgs, Span),
    UnsupportedArg(SpannedInnerArgs, BuiltinTypeKind),
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
