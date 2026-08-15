use chrn_utils::id_types::{ModuleId, ScopeId, SymbolId};
use lang::chrn_classifier::{ChrnClassifiable, ChrnClassifier};

use crate::semantic::hir::{hir_concepts::Table, hir_symbols::SymbolKindFlat};

//TODO: Maybe this is the point where the scope wrapper comes in
/// Structure that holds scope data
#[derive(Debug)]
pub struct ScopeInfo {
    pub scope: Scope,
    /// For debugging purposes so that the symbol of origin is known for where a namespace lookup
    /// occured, beyond just the module or scope of origin.
    pub sym_owner: Option<SymbolId>,
    pub mod_owner: ModuleId,
}

impl ScopeInfo {
    pub fn new(scope: Scope, sym_owner: Option<SymbolId>, mod_owner: ModuleId) -> ScopeInfo {
        ScopeInfo {
            scope,
            sym_owner,
            mod_owner,
        }
    }
}

pub const SCOPE_CORE: u8 = 1 << 0;
pub const SCOPE_NEUTRAL: u8 = 1 << 1;
pub const SCOPE_VAR: u8 = 1 << 2;
pub const SCOPE_NEST: u8 = 1 << 3;
pub const SCOPE_COMPLEX: u8 = 1 << 4;
pub const SCOPE_OVERRIDE: u8 = 1 << 5;
pub const SCOPE_LOCAL: u8 = 1 << 6;
pub const SCOPE_COMPILER: u8 = 1 << 7;

//NOTE: These must always end with Core so that the "- 1" semantics work when `NamespaceOnly` is picked.
//Question mark

pub static SCOPE_CORE_ENCODED_SCOPES: [ScopeType; 1] = [ScopeType::Core];

/// Elements ordered to fit the languages rules of section `neutral`
pub static SCOPE_NEUTRAL_ENCODED_SCOPES: [ScopeType; 3] =
    [ScopeType::Neutral, ScopeType::Compiler, ScopeType::Core];

/// Elements ordered to fit the languages rules of section `var`
pub static SCOPE_VAR_ENCODED_SCOPES: [ScopeType; 4] = [
    ScopeType::Nest,
    ScopeType::Neutral,
    ScopeType::Compiler,
    ScopeType::Core,
];

/// Elements ordered to fit the languages rules of section `nest`
pub static SCOPE_NEST_ENCODED_SCOPES: [ScopeType; 5] = [
    ScopeType::Var,
    ScopeType::Nest,
    ScopeType::Neutral,
    ScopeType::Compiler,
    ScopeType::Core,
];

// Doesn't have itself because complex assigns properties for types, nothing more.
/// Elements ordered to fit the needs of scope `complex`
pub static SCOPE_COMPLEX_ENCODED_SCOPES: [ScopeType; 5] = [
    ScopeType::Var,
    ScopeType::Nest,
    ScopeType::Neutral,
    ScopeType::Compiler,
    ScopeType::Core,
];

/// Elements ordered to fit the needs of scope `override`
pub static SCOPE_OVERRIDE_ENCODED_SCOPES: [ScopeType; 6] = [
    ScopeType::Override,
    ScopeType::Var,
    ScopeType::Nest,
    ScopeType::Neutral,
    ScopeType::Compiler,
    ScopeType::Core,
];

//WARN: Suspicious accessibility
pub static SCOPE_LOCAL_ENCODED_SCOPES: [ScopeType; 1] = [ScopeType::Local];
pub static SCOPE_VAR_ONLY: [ScopeType; 1] = [ScopeType::Var];
pub static SCOPE_NEST_ONLY: [ScopeType; 1] = [ScopeType::Nest];

// Neutral, var, nest, and complex scopes can only access variables from neutral and nest.
// Override is unsure
#[derive(Debug)]
pub struct Scope {
    pub table: Table,
    /// Own `ScopeId`
    pub scope_id: ScopeId,
    /// `ScopeType` this scope represents
    pub scope_type: ScopeType,
    /// An `Option` scope that is intrinsically a part of this scope
    pub intrinsic_scope: Option<ScopeId>,
    /// Boolean of whether or not the scope is intrinsic
    pub is_intrinsic: bool,
    /// Pre-determined list of `SceopType`s that this scope can access.
    pub accessible_scopes: &'static [ScopeType],
}

impl Scope {
    pub(crate) fn new(
        scope_id: ScopeId,
        scope_type: ScopeType,
        is_intrinsic: bool,
        intrinsic_scope: Option<ScopeId>,
    ) -> Scope {
        let accessible_scopes = scope_type.accessible_scopes();
        Scope {
            table: Table::new(),
            scope_id,
            scope_type,
            intrinsic_scope,
            accessible_scopes,
            is_intrinsic,
            // pub visible_scopes: Vec<ScopeId>,
        }
    }

    pub(crate) fn with_table(
        scope_id: ScopeId,
        scope_type: ScopeType,
        intrinsic_scope: Option<ScopeId>,
        is_intrinsic: bool,
        table: Table,
    ) -> Scope {
        let accessible_scopes = scope_type.accessible_scopes();
        Scope {
            table,
            scope_id,
            scope_type,
            is_intrinsic,
            intrinsic_scope,
            accessible_scopes,
        }
    }
}

#[derive(Debug)]
pub struct SymbolLookupOutput {
    pub found_sym_id: SymbolId,
    pub scope_found_in: ScopeId,
}

impl SymbolLookupOutput {
    pub fn new(found_sym_id: SymbolId, scope_found_in: ScopeId) -> SymbolLookupOutput {
        SymbolLookupOutput {
            found_sym_id,
            scope_found_in,
        }
    }
}

/// Enum representing all kinds of scopes usable in chrn
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ScopeType {
    Compiler,
    Core,
    Local,
    Neutral,
    Var,
    Nest,
    Complex,
    Override,
}

impl ScopeType {
    /// Direct representation of how the language views scope accessibility.
    /// `needs_global` purely exists for all scope accessibility reasons
    pub fn accessible_scopes(self) -> &'static [ScopeType] {
        match self {
            ScopeType::Core => &SCOPE_CORE_ENCODED_SCOPES,
            // Mainly for internal usage, not an actual program recognizable scope
            // Neutral can only access neutral because this section is purely for declaring and
            // using in other sections
            ScopeType::Neutral => &SCOPE_NEUTRAL_ENCODED_SCOPES,
            ScopeType::Var => &SCOPE_VAR_ENCODED_SCOPES,
            ScopeType::Override => &SCOPE_OVERRIDE_ENCODED_SCOPES,
            ScopeType::Nest => &SCOPE_NEST_ENCODED_SCOPES,
            ScopeType::Complex => &SCOPE_COMPLEX_ENCODED_SCOPES,
            ScopeType::Local => &SCOPE_LOCAL_ENCODED_SCOPES,
            // Should be a recognized builtin at this point
            ScopeType::Compiler => &[],
        }
    }

    pub(crate) fn to_u8(self) -> u8 {
        match self {
            ScopeType::Core => SCOPE_CORE,
            ScopeType::Neutral => SCOPE_NEUTRAL,
            ScopeType::Var => SCOPE_VAR,
            ScopeType::Nest => SCOPE_NEST,
            ScopeType::Complex => SCOPE_COMPLEX,
            ScopeType::Override => SCOPE_OVERRIDE,
            ScopeType::Local => SCOPE_LOCAL,
            ScopeType::Compiler => SCOPE_COMPILER,
        }
    }

    pub(crate) fn has_intrinsic_scope(self) -> bool {
        match self {
            ScopeType::Override => true,
            ScopeType::Complex |
            ScopeType::Core
            | ScopeType::Local
            | ScopeType::Neutral
            | ScopeType::Nest
            // I don't know does it?
            | ScopeType::Compiler
            | ScopeType::Var => false,
        }
    }
}

// Maybe Only(ScopeType)
/// This enum is intended to disallow core defined values from being searched for when syntax such
/// as "main::i32" is used. i32 is not owned by main, but innately main is attached to core, meaning
/// without the explicit noting of whether we are searching a singular module's namespace it would
/// innately allow for main.i32 to be interpreted the same as if just i32 was written, which is
/// wrong since the namespace "main" owns no such thing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeLookupPattern {
    /// Applies no restriction to lookups. Meaning, core is automatically searched since it's
    /// intrinsic, any scope's accessible scopes can be searched with no restriction.
    NoRestrictions,
    // WHat
    /// Restricts lookup to only search what is within the given namespace, which restricts modules
    /// such as core, or anything not declared within the symbol's scope containment?
    NamespaceOnly,
    /// Lookup that not only allows for `nest` to be searched, but also enforces it's the only section
    /// that can be searched
    OnlyNest,
    /// Lookup that not only allows for `var` to be searched, but also enforces it's the only section
    /// that can be searched
    OnlyVar,
}

// TODO: Formattable
impl std::fmt::Display for ScopeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScopeType::Core => write!(f, "core"),
            ScopeType::Neutral => write!(f, "neutral"),
            ScopeType::Var => write!(f, "var"),
            ScopeType::Nest => write!(f, "nest"),
            ScopeType::Complex => write!(f, "complex"),
            ScopeType::Override => write!(f, "override"),
            ScopeType::Local => write!(f, "local"),
            ScopeType::Compiler => write!(f, "compiler"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Scope type specifically for if a symbol has an associated scope tied to it
pub enum AssociatedScopeKind {
    // A bit redundant since odules already hold themselves as a scope
    /// Meaning the scope is inside of a module's vector of `ScopeId`
    Module(ModuleId),
    /// Meaning the scope is just attached to a symbol's namespace
    Scope(ScopeId),
}

impl ChrnClassifiable for AssociatedScopeKind {
    fn to_classified(&self) -> lang::chrn_classifier::ChrnClassifier {
        match self {
            AssociatedScopeKind::Module(_) => ChrnClassifier::Module,
            AssociatedScopeKind::Scope(_) => ChrnClassifier::Namespace,
        }
    }
}

pub struct IntrinsicRegistry {
    pub core_mod_id: ModuleId,
    pub override_scope_id: Option<ScopeId>,
}

impl IntrinsicRegistry {
    pub fn new(core_mod_id: ModuleId, override_scope_id: Option<ScopeId>) -> IntrinsicRegistry {
        IntrinsicRegistry {
            core_mod_id,
            override_scope_id,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct ScopeLookupPreferenceFlags {
    /// If `None`, no preference is accounted for meaning all preference checks are `true`
    flags: Option<u16>,
}

// This is more like a general purpose set of flags since it doesn't really matter if flat kinds are
// used or not
impl ScopeLookupPreferenceFlags {
    pub const TYPE: u16 = 1 << 0;
    pub const VARIABLE: u16 = 1 << 1;
    pub const NAMESPACE: u16 = 1 << 2;
    pub const DIRECTIVE: u16 = 1 << 3;
    pub const EXTERN_TYPE: u16 = 1 << 4;

    pub fn new(flags: Option<u16>) -> Self {
        Self { flags }
    }

    /// Creates lookup preference with no preferred options
    pub fn none() -> ScopeLookupPreferenceFlags {
        ScopeLookupPreferenceFlags::new(None)
    }

    pub fn is_none(self) -> bool {
        self.flags.is_none()
    }

    /// Checks if the `SymbolKindFlat` converted to a valid set of bits for `LookupPreferenceFlags`
    /// is contained within `self`
    pub fn is_preferred(self, kind: SymbolKindFlat) -> bool {
        if let Some(flags) = self.flags {
            flags & flat_sym_kind_to_preferred_bits(kind) != 0
        } else {
            // No options chosen. Anything attempted to be matched to a `None` preference succeeds.
            true
        }
    }
}

/// Local function to turn `SymbolKindFlat` into a preferred option.
/// This exists because the `to_bits()` from flat symbols are just direct mappings, meaning there is
/// no signifying bit usable to say "No options selected", hence the explicit translation layer here.
pub(super) const fn flat_sym_kind_to_preferred_bits(kind: SymbolKindFlat) -> u16 {
    match kind {
        SymbolKindFlat::Type => ScopeLookupPreferenceFlags::TYPE,
        SymbolKindFlat::Variable => ScopeLookupPreferenceFlags::VARIABLE,
        SymbolKindFlat::Namespace => ScopeLookupPreferenceFlags::NAMESPACE,
        SymbolKindFlat::Directive => ScopeLookupPreferenceFlags::DIRECTIVE,
        SymbolKindFlat::ExternType => ScopeLookupPreferenceFlags::EXTERN_TYPE,
    }
}
