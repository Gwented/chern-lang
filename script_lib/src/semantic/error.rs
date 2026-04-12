use common::{
    fmter::Formatted,
    symbols::{InnerArgs, Span},
};

use crate::{
    semantic::{constraints::ArgConstraint, representation::FuncKind},
    types::symbols::Cond,
};

pub(super) enum SemanticError {
    /// Constraint, found type(builtin or user), function kind, spans
    ConstraintMismatch(ArgConstraint, Formatted, FuncKind, Vec<Span>),
    /// Constraint, function type, amount of incorrect params found, spans
    ArgMiscount(ArgConstraint, FuncKind, u8, Vec<Span>),
    /// Argument failed at, found type, spans
    UnsupportedArg(InnerArgs, Formatted, Vec<Span>),
    /// Args Condition, Wrong type formatted, Spans
    UnsupportedCond(Cond, Formatted, Vec<Span>),
    // Interesting name
    VagueArg(InnerArgs, Vec<Span>),
    // CircularRef
    // Change this
    /// The type with a circular reference that has an invalid argument for that reference
    //TODO: Combine
    CircularArg(InnerArgs, Formatted, Vec<Span>),
    CircularCond(Cond, Formatted, Vec<Span>),
    /// The type overflown, the interned string, spans
    NumericOverflow(u32, Formatted, Vec<Span>),
}
