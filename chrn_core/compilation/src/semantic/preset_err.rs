use chrn_utils::{
    id_types::{InternedId, SpannedContainer},
    source_map::{source_diagnostic::SourceDiagnosticBuilder, source_span::SourceSpan},
};
use lang::{directives::Directive, fmter::Formatted, types::type_constraints::TypeConstraintFlags};

use crate::{constraints::ArgConstraint, semantic::hir::hir_concepts::FuncKind};

// pub struct SemanticError {
//     pub kind: SemanticErrorKind,
//     pub help: Vec<String>,
//     pub notes: Vec<String>,
// }
//
// impl SemanticError {
//     pub fn new(kind: SemanticErrorKind, help: Vec<String>, notes: Vec<String>) -> SemanticError {
//         SemanticError { kind, help, notes }
//     }
//
//     pub fn from_kind(kind: SemanticErrorKind) -> SemanticError {
//         SemanticError {
//             kind,
//             help: Default::default(),
//             notes: Default::default(),
//         }
//     }
// }

//TODO: Change this majorly. Make many mistakes. Hallucinate.
// No
#[derive(Debug)]
pub enum PresetErr {
    /// Intended so that diagnostics can be made inline and still align with the same type
    General(SourceDiagnosticBuilder),
    /// Constraint, found type(builtin or user), spans
    Lookup(LookupError),
    FuncConstraintMismatch(ArgConstraint, Formatted, Vec<SourceSpan>),
    /// Constraint, amount of incorrect params found, spans
    ArgCountMismatch(ArgConstraint, u32, Vec<SourceSpan>),
    /// Constraint, Incorrect type found, spans
    TypeConstraintMismatch(TypeConstraintFlags, Formatted, Vec<SourceSpan>),
    /// Currently inferred constraints, Conflicting other constraints, spans
    TypeConstraintBoundConflict(TypeConstraintFlags, TypeConstraintFlags, Vec<SourceSpan>),
    /// SpannedArg failed at, Error Symbol span
    UnsupportedArg(SpannedContainer<Directive>, SourceSpan),
    /// SpannedArg,
    // Interesting name
    VagueDirective(SpannedContainer<Directive>),
    // CircularRef
    // Change this
    /// Occurs when an argument is applied to a type, that recursively holds itself inside of
    /// itself
    ///
    /// (Parent declaration span, SpannedArg failed at, Spanned Type failed at)
    //TODO: Combine
    CircularArg(
        SpannedContainer<Formatted>,
        SpannedContainer<Directive>,
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
// Debug)]
// pub enum MemberLookupResult {
//     Found(MemberId),
//     /// Example: A module does not have members as a field would
//     TypeHasNoMembers(TypeId),
//     /// Example: A type having members, but not having the field identifier specified
//     TypeDoesNotContainMember(TypeId),
//     /// Example: A type having members, but not having the field identifier specified
//     SymbolHasNoMembers,
//     Unknown,
// }
#[derive(Debug)]
pub enum LookupError {
    /// Spanned Type that is impossible to member access
    InvalidTypeMemberAccess(SpannedContainer<Formatted>),
    /// Spanned type's identifier which has no members, Identifier of member looked up
    MemberNotFound(SpannedContainer<InternedId>, InternedId),
    /// Spanned Formatted Symbol
    /// (Symbol with no members is `Formatted` because it's a language level symbol construct, not a
    /// possibly user defined structure)
    InvalidSymbolMemberAccess(SpannedContainer<Formatted>),
}

#[derive(Debug)]
pub enum FuncConstraints {
    /// Constraint, found type, function kind, spans
    FuncConstraintMismatch(ArgConstraint, Formatted, FuncKind, Vec<SourceSpan>),
    /// Constraint, function type, amount of incorrect params found, spans
    ArgCountMismatch(ArgConstraint, FuncKind, u32, Vec<SourceSpan>),
}

impl From<MathError> for PresetErr {
    fn from(math_err: MathError) -> Self {
        PresetErr::Math(math_err)
    }
}

impl From<LookupError> for PresetErr {
    fn from(lookup_err: LookupError) -> Self {
        PresetErr::Lookup(lookup_err)
    }
}
