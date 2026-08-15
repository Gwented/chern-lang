use chrn_utils::{
    id_types::{InternedId, ModuleId, SpannedContainer, SymbolId, TypeId},
    source_map::{source_diagnostic::SourceDiagnosticBuilder, source_span::SourceSpan},
};
use lang::{
    chrn_classifier::ChrnClassifier, directives::Directive, types::boundaries::TypeBoundaryFlags,
    values::ValueKind,
};

use crate::{
    constraints::ArgConstraint,
    lookup::scopes::scopes_concepts::AssociatedScopeKind,
    parser::ast::ast_concepts::{BinaryOp, UnaryOp},
    semantic::hir::hir_symbols::FuncKind,
};

#[derive(Debug)]
pub enum PresetErr {
    /// Intended so that diagnostics can be made inline and still align with the same type
    General(SourceDiagnosticBuilder),
    /// Constraint, found type(builtin or user), spans
    Lookup(LookupError),
    FuncConstraintMismatch {
        constraint: ArgConstraint,
        fmtted_ty: ChrnClassifier,
        spans: Vec<SourceSpan>,
    },
    /// Spanned Directive
    UnknownDirective(SpannedContainer<InternedId>),
    // TypecheckFailed {
    //     invalid_ty: SpannedContainer<TypeId>,
    // },
    /// Constraint, amount of incorrect params found, spans
    DirectiveCountMismatch {
        constraint: ArgConstraint,
        count: u32,
        spans: Vec<SourceSpan>,
    },
    /// Constraint, Incorrect type found, spans
    TypeBoundaryMismatch {
        given_constraints: TypeBoundaryFlags,
        found_ty: ChrnClassifier,
        spans: Vec<SourceSpan>,
    },
    /// Duplicate identiiers were found
    DuplicateIdents {
        sp_original: SpannedContainer<InternedId>,
        sp_dup: SpannedContainer<InternedId>,
        /// What the duplicate actually was.
        /// Like if it should output "duplicate field/variant/parameter" etsy
        classifier: ChrnClassifier,
    },
    /// Currently inferred constraints, Conflicting other constraints, spans
    TypeBoundaryBoundConflict {
        inferred: TypeBoundaryFlags,
        conflicting: TypeBoundaryFlags,
        spans: Vec<SourceSpan>,
    },
    /// SpannedArg failed at, Error Symbol span
    UnsupportedDirective {
        sp_directive: SpannedContainer<Directive>,
        sym_span: SourceSpan,
    },
    /// SpannedArg,
    // Interesting name
    VagueDirective(SpannedContainer<Directive>),
    // CircularRef
    // Change this
    /// Occurs when an argument is applied to a type, that recursively holds itself inside of
    /// itself
    ///
    /// (Parent declaration span, Spanned directive failed at, Type span failed at)
    //TODO: Combine
    CircularDirective {
        sp_fmtted_parent: SpannedContainer<ChrnClassifier>,
        // Actual parent name
        // SpannedContainer<InternedId>,
        sp_directive: SpannedContainer<Directive>,
        err_ty_span: SourceSpan,
    },
    /// SpannedInterned number, type overflown
    //WARN: This technically shouldn't exist since BigInt/BigFloat would exist
    NumericOverflow {
        sp_num: SpannedContainer<InternedId>,
        fmtted_ty: ChrnClassifier,
    },
    //TODO: Maybe option name id?
    UndefinedMember(SourceSpan),
    Math(MathError),
    // Schema(*const u8),
}

// #[derive(Debug)]
// pub enum SchemaError {
//     BoundaryMismatch {
//         sp_err_boundaries: SpannedContainer<TypeBoundaryFlags>,
//         required_boundaries: TypeBoundaryFlags,
//     },
// }

#[derive(Debug)]
pub enum MathError {
    /// Spanned lhs, Spanned rhs, Op
    BinaryOpMismatch {
        sp_lhs: SpannedContainer<ValueKind>,
        sp_rhs: SpannedContainer<ValueKind>,
        op: BinaryOp,
    },
    /// Spanned Operand, operator
    UnaryOpMismatch {
        sp_operand: SpannedContainer<ValueKind>,
        op: UnaryOp,
    },
    /// lhs span, rhs span
    DivideByZero {
        lhs_span: SourceSpan,
        rhs_span: SourceSpan,
    },
}

// MemberLookupError, SymbolLookupError?
#[derive(Debug)]
pub(crate) enum LookupError {
    /// Search context was given an identifier and scope, but the identifier does not exist in the
    /// given scope.
    SymbolNotFound {
        sp_invalid_name_id: SpannedContainer<InternedId>,
        scope_searched: AssociatedScopeKind,
    },
    /// Search context was expecting a type but found a non-type
    NotAType {
        invalid_sym_id: SymbolId,
        sp_invalid_name_id: SpannedContainer<InternedId>,
        scope_found_in: AssociatedScopeKind,
    },
    /// Search context found a type, but it isn't accessible from `current_mod_id`
    PrivateTypeAccess {
        sp_found_type_id: SpannedContainer<TypeId>,
        found_sym_id: SymbolId,
        current_mod_id: ModuleId,
    },
    /// Spanned Type that is impossible to member access
    ImpossibleTypeMemberAccess(SpannedContainer<ChrnClassifier>),
    /// Spanned type's identifier which has no members, Identifier of member looked up
    MemberNotFound {
        parent_type_id: TypeId,
        sp_parent_name_id: SpannedContainer<InternedId>,
        /// Member looked up but not found
        sp_not_found: InternedId,
    },
    /// Spanned Formatted Symbol
    /// (Symbol with no members is `Formatted` because it's a language level symbol construct, not a
    /// possibly user defined structure)
    InvalidSymbolMemberAccess(SpannedContainer<ChrnClassifier>),
}

#[derive(Debug)]
pub enum FuncConstraints {
    /// Constraint, found type, function kind, spans
    FuncConstraintMismatch {
        constraint: ArgConstraint,
        fmtted_ty: ChrnClassifier,
        func_kind: FuncKind,
        spans: Vec<SourceSpan>,
    },
    /// Constraint, function type, amount of incorrect params found, spans
    ArgCountMismatch {
        constraint: ArgConstraint,
        func_kind: FuncKind,
        count: u32,
        spans: Vec<SourceSpan>,
    },
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
