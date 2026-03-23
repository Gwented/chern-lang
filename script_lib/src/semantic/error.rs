use common::{
    builtins::BuiltinTypeKind,
    fmter::{Formatable, Formatted},
    symbols::{InnerArgs, Span, SpannedInnerArgs, TypeId},
};

use crate::semantic::{constraints::ArgConstraint, representation::FuncKind};

// Lifetimes
//NOTE: Taking in Vec may change
pub(super) enum SemanticError {
    // constraint, found type, what function type, span
    ConstraintMismatch(ArgConstraint, Formatted, FuncKind, Vec<Span>),
    /// Constraint, function type, amount of incorrect params found, span
    ArgMiscount(ArgConstraint, FuncKind, u8, Vec<Span>),
    // argument failed at, found type
    //TODO: Maybe shouldn't force spanned inner args here
    UnsupportedArg(InnerArgs, Formatted, Vec<Span>),
    // Interesting name
    VagueArg(InnerArgs, Vec<Span>),
    // CircularRef
    // The type with a circular reference that has an invalid argument for that reference
    CircularRef(InnerArgs, Formatted, Vec<Span>),
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
