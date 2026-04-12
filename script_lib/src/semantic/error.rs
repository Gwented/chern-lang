use common::{
    fmter::Formatted,
    symbols::{InnerArgs, Span},
};

use crate::{
    semantic::{constraints::ArgConstraint, representation::FuncKind},
    types::symbols::Cond,
};

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

    ///@Args Condition, Wrong type formatted, Spans
    UnsupportedCond(Cond, Formatted, Vec<Span>),
    // Interesting name
    VagueArg(InnerArgs, Vec<Span>),
    // CircularRef
    // The type with a circular reference that has an invalid argument for that reference
    //TODO: Combine
    CircularArg(InnerArgs, Formatted, Vec<Span>),
    CircularCond(Cond, Formatted, Vec<Span>),
}
