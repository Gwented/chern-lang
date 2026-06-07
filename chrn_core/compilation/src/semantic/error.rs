use chrn_utils::{
    fmter::Formatted,
    id_types::{InternedId, SpannedContainer},
    source_map::{source_diagnostic::SourceDiagnostic, source_span::SourceSpan},
};
use lang::{inner_args::InnerArgs, types::type_constraints::TypeConstraintFlags};

use crate::{constraints::ArgConstraint, semantic::hir::FuncKind};

//TODO: Change this majorly. Make many mistakes. Hallucinate.
#[derive(Debug)]
pub enum SemanticError {
    /// Intended so that diagnostics can be made inline and still align with the same type
    General(SourceDiagnostic),
    /// Constraint, found type(builtin or user), spans
    FuncConstraintMismatch(ArgConstraint, Formatted, Vec<SourceSpan>),
    /// Constraint, amount of incorrect params found, spans
    ArgCountMismatch(ArgConstraint, u32, Vec<SourceSpan>),
    /// Constraint, Incorrect type found, spans
    TypeConstraintMismatch(TypeConstraintFlags, Formatted, Vec<SourceSpan>),
    /// Currently inferred constraints, Conflicting other constraints, spans
    TypeConstraintBoundConflict(TypeConstraintFlags, TypeConstraintFlags, Vec<SourceSpan>),
    /// SpannedArg failed at, Error Symbol span
    UnsupportedArg(SpannedContainer<InnerArgs>, SourceSpan),
    /// SpannedArg,
    // Interesting name
    VagueArg(SpannedContainer<InnerArgs>),
    // CircularRef
    // Change this
    /// Occurs when an argument is applied to a type, that recursively holds itself inside of
    /// itself
    ///
    /// (Parent declaration span, SpannedArg failed at, Spanned Type failed at)
    //TODO: Combine
    CircularArg(
        SpannedContainer<Formatted>,
        SpannedContainer<InnerArgs>,
        SourceSpan,
    ),
    /// SpannedInterned number, type overflown
    //WARN: This technically shouldn't exist since BigInt/BigFloat would exist
    NumericOverflow(SpannedContainer<InternedId>, Formatted),
    //TODO: Maybe option name id?
    UndefinedMember(SourceSpan),
    Math(MathError),
}

#[derive(Debug)]
pub enum MathError {
    /// SpannedLhs, SpannedRhs, Op
    BinaryOpMismatch(
        SpannedContainer<Formatted>,
        SpannedContainer<Formatted>,
        Formatted,
    ),
    /// Spanned Operand, operator, spans
    UnaryOpMismatch(SpannedContainer<Formatted>, Formatted),
    /// Lhs, rhs, spans
    DivideByZero(Formatted, Vec<SourceSpan>),
}

#[derive(Debug)]
pub(super) enum FunctionConstraints {
    /// Constraint, found type, function kind, spans
    FuncConstraintMismatch(ArgConstraint, Formatted, FuncKind, Vec<SourceSpan>),
    /// Constraint, function type, amount of incorrect params found, spans
    ArgCountMismatch(ArgConstraint, FuncKind, u32, Vec<SourceSpan>),
}

impl From<MathError> for SemanticError {
    fn from(math_err: MathError) -> Self {
        SemanticError::Math(math_err)
    }
}
