use chrn_utils::{inner_args::InnerArgs, types::type_constraints::TypeConstraintFlags};
use common::{fmter::Formatted, span::Span};

use crate::semantic::{constraints::ArgConstraint, representation::FuncKind};

//TODO: Change this majorly. Make many mistakes. Hallucinate.
#[derive(Debug)]
pub enum SemanticError {
    /// msg, spans
    General(String, Vec<Span>),
    /// Constraint, found type(builtin or user), spans
    FuncConstraintMismatch(ArgConstraint, Formatted, Vec<Span>),
    /// Constraint, amount of incorrect params found, spans
    ArgCountMismatch(ArgConstraint, u32, Vec<Span>),
    /// Constraint, Incorrect type found, spans
    TypeConstraintMismatch(TypeConstraintFlags, Formatted, Vec<Span>),
    /// Currently inferred constraints, Conflicting other constraints, spans
    TypeConstraintBoundConflict(TypeConstraintFlags, TypeConstraintFlags, Vec<Span>),
    /// Argument failed at, spans
    UnsupportedArg(InnerArgs, Vec<Span>),
    /// Args Condition, Wrong type formatted, Spans
    // Interesting name
    VagueArg(InnerArgs, Vec<Span>),
    // CircularRef
    // Change this
    /// The type with a circular reference that has an invalid argument for that reference
    //TODO: Combine
    CircularArg(InnerArgs, Formatted, Vec<Span>),
    /// The interned string, type overflown, spans
    //WARN: This technically shouldn't exist since BigInt/BigFloat would exist
    NumericOverflow(u32, Formatted, Vec<Span>),
    //TODO: Maybe option name id?
    UndefinedMember(Span),
    Math(MathError),
}

#[derive(Debug)]
pub enum MathError {
    /// Lhs, rhs, op, spans
    BinaryOpMismatch(Formatted, Formatted, Formatted, Vec<Span>),
    /// operand, op, spans
    UnaryOpMismatch(Formatted, Formatted, Vec<Span>),
    /// Lhs, rhs, spans
    DivideByZero(Formatted, Vec<Span>),
}

#[derive(Debug)]
pub(super) enum FunctionConstraints {
    /// Constraint, found type, function kind, spans
    FuncConstraintMismatch(ArgConstraint, Formatted, FuncKind, Vec<Span>),
    /// Constraint, function type, amount of incorrect params found, spans
    ArgCountMismatch(ArgConstraint, FuncKind, u32, Vec<Span>),
}

impl From<MathError> for SemanticError {
    fn from(math_err: MathError) -> Self {
        SemanticError::Math(math_err)
    }
}
