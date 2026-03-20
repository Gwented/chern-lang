use common::{
    builtins::BuiltinTypeKind,
    symbols::{InnerArgs, Span, SpannedInnerArgs, TypedId},
};

use crate::semantic::representation::{ArgConstraint, FuncKind};

#[derive(Debug)]
// Lifetimes
pub(super) enum SemanticError {
    // constraint, found type, what function type, span
    TypeMismatch(ArgConstraint, BuiltinTypeKind, FuncKind, Span),
    // constraint, function type, params found, span
    ParamMiscount(ArgConstraint, FuncKind, u8, Span),
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
