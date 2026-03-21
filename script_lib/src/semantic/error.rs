use common::{
    builtins::BuiltinTypeKind,
    fmter::{Formatable, Formatted},
    symbols::{InnerArgs, Span, SpannedInnerArgs, TypeId},
};

use crate::semantic::{constraints::ArgConstraint, representation::FuncKind};

// Lifetimes
pub(super) enum SemanticError {
    // constraint, found type, what function type, span
    ConstraintMismatch(ArgConstraint, Formatted, FuncKind, Span),
    /// Constraint, function type, amount of incorrect params found, span
    ArgMiscount(ArgConstraint, FuncKind, u8, Span),
    // argument failed at, found type
    //TODO: Maybe shouldn't force spanned inner args here
    UnsupportedArg(SpannedInnerArgs, Formatted),
    // Interesting name
    VagueArg(InnerArgs, Span),
    // CircularRef
    // The type with a circular reference that has an invalid argument for that reference
    CircularRef(InnerArgs, Formatted, Span),
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
