use chern_core::inner_args::InnerArgs;
use common::{fmter::Formatted, span::Span};

use crate::{
    conditions::Cond,
    semantic::{constraints::ArgConstraint, representation::FuncKind},
};

//TODO: Change this majorly. Make many mistakes. Hallucinate.
pub(super) enum SemanticError {
    /// Constraint, found type(builtin or user), function kind, spans
    FuncConstraintMismatch(ArgConstraint, Formatted, FuncKind, Vec<Span>),
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
    /// The interned string, type overflown, spans
    NumericOverflow(u32, Formatted, Vec<Span>),
    Math(MathError),
}

pub(super) enum MathError {
    /// Lhs, rhs, Op, spans
    BinaryOpMismatch(Formatted, Formatted, Formatted, Vec<Span>),
    /// operand, op, spans
    UnaryOpMismatch(Formatted, Formatted, Vec<Span>),
}

impl From<MathError> for SemanticError {
    fn from(math_err: MathError) -> Self {
        SemanticError::Math(math_err)
    }
}
