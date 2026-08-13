use chrn_utils::{
    id_types::{
        AstId, ConfigRootId, ExprId, ImplId, ImplMemberId, InternedId, MemberId, SpannedContainer,
        SymbolId, TypeId,
    },
    source_map::source_span::SourceSpan,
};

use crate::{
    lookup::scopes::{ScopeLookupPattern, ScopeType},
    parser::ast::{ast_concepts::ConfigRootKindFlat, ast_exprs::TypeExpr},
};

#[derive(Debug)]
pub struct ImplHir {
    pub impl_id: ImplId,
    pub kind: ImplHirKind,
    pub scope_origin: ScopeType,
    pub ast_id: Option<AstId>,
}

impl ImplHir {
    pub fn new(
        impl_id: ImplId,
        kind: ImplHirKind,
        scope_origin: ScopeType,
        ast_id: Option<AstId>,
    ) -> Self {
        Self {
            impl_id,
            kind,
            scope_origin,
            ast_id,
        }
    }
}

#[derive(Debug)]
pub enum ImplHirKind {
    Config(ConfigRootId),
}

#[derive(Debug)]
pub enum ImplMemberKind {
    /// `ConfigDefMember`
    ConfigDefMember(ConfigDefMember),
    /// Root specific option assignment
    OptAssignmentRoot(OptionAssignmentRoot),
    /// Member specific option assignment
    OptAssignmentMember(OptionAssignmentMember),
    /// Takes lhs types, and assigns to a single rhs `SymbolKind::ExternType`
    MultiTypeAssignment(MultiTypeAssignment),
    /// Member that has reserved a slot but not yet defined
    Unknown {
        sp_name_id: SpannedContainer<InternedId>,
        reserved_member_id: ImplMemberId,
    },
}

#[derive(Debug)]
pub enum ConfigRootKind {
    Complex(ConfigRootComplex),
    Override(ConfigRootOverride),
}

impl ImplMemberKind {
    pub fn is_unknown(&self) -> bool {
        match self {
            ImplMemberKind::Unknown { .. } => true,
            _ => false,
        }
    }
}

/// Common inner of config roots
#[derive(Debug)]
pub struct ConfigRootCommon {
    /// `ImplId` of `self`
    pub impl_id: ImplId,
    /// ConfigId of `self`
    pub cfg_root_id: ConfigRootId,
    /// Lookup pattern that needs to be used to properly discern if
    /// `ScopeLookupPattern::Namespace/OnlyVar` should be used to search for the symbol associated with
    /// thie config
    pub lookup_pat: ScopeLookupPattern,
    // /// ISOLATE
    // pub kind: ConfigRootKindFlat,
    /// Expects `ConfigDefMember`
    pub cfg_members: Vec<ImplMemberId>,
}

impl ConfigRootCommon {
    pub fn new(
        impl_id: ImplId,
        cfg_root_id: ConfigRootId,
        lookup_pat: ScopeLookupPattern,
        // kind: ConfigRootKindFlat,
        cfg_members: Vec<ImplMemberId>,
    ) -> Self {
        Self {
            impl_id,
            cfg_root_id,
            lookup_pat,
            // kind,
            cfg_members,
        }
    }
}

// TODO: Readiness for skipping during resolution
/// Intended to represent a config block environment that consumes options for a field.
/// Specifically tied to `overrid` section semantics
#[derive(Debug)]
pub struct ConfigRootOverride {
    pub common: ConfigRootCommon,
    /// During name resolution, we can't actually lookup the symbol since it may or may not be
    /// registered, so it's Option since it actually is `None` at some point, and could remain
    /// `None` if in a later stage it doesn't have it's target symbol found.
    /// Must be `Namespace` or `Type`
    //NOTE: Can only be either a type id or namespace. So maybe um...um....!
    pub linked_sym_id: Option<SymbolId>,
    /// Expects `OptionAssignmentRoot`
    pub opt_assignments: Vec<ImplMemberId>,
}

impl ConfigRootOverride {
    pub fn new(
        common: ConfigRootCommon,
        linked_sym_id: Option<SymbolId>,
        opt_assignments: Vec<ImplMemberId>,
    ) -> ConfigRootOverride {
        ConfigRootOverride {
            common,
            linked_sym_id,
            opt_assignments,
        }
    }
}

/// Intended to represent a config block environment that consumes options for a field.
/// Specifically tied to `complex` section semantics
#[derive(Debug)]
pub struct ConfigRootComplex {
    pub common: ConfigRootCommon,
    /// Type only
    pub linked_type_id: Option<TypeId>,
    pub impl_stmts: Vec<ImplMemberId>,
}

impl ConfigRootComplex {
    pub fn new(
        common: ConfigRootCommon,
        linked_type_id: Option<TypeId>,
        impl_stmts: Vec<ImplMemberId>,
    ) -> Self {
        Self {
            common,
            linked_type_id,
            impl_stmts,
        }
    }
}

//WARN: Should these be different structures? OverrideConfigMember? ComplexConfigMember?

/// The member inside of a `ConfigDef` or `ConfigDefMember` which is the same structure,
/// but with ties to an `ImplMemberKind` instead of a `ImplHir`
#[derive(Debug)]
pub struct ConfigDefMember {
    /// Is a name id instead of symbol id since `NameResolver` merely registers names, with no
    /// knowledge of symbol specifics. A dependency system may be used in the future.
    pub name_id: InternedId,
    // This is not a `SpannedContainer` because it may become an Option
    pub name_span: SourceSpan,
    /// `ImplMemberId` of `self`
    pub impl_member_id: ImplMemberId,
    /// `MemberId` of the member symbol this is attached to
    pub linked_member_id: MemberId,
    // This is mostly here because the padding is going to make it 80 bytes anyways so why not store
    // the type to avoid extra lookups
    /// `TypeId` of the member symbol this is attached to
    /// This is `Option` because a type like a variant doesn't have a type, so this is not
    /// guaranteed
    pub linked_member_type_id: Option<TypeId>,
    /// Expects `OptionAssignmentMember`
    pub opt_assignments: Vec<ImplMemberId>,
    // These configs are supposed to be usable by override too so maybe this becomes an enum where
    // it exposes metadata depending on override or not.
    pub metadata: ConfigMemberMetadataKind,
    //NOTE: Members use `ScopeLookupPattern::NamespaceOnly` only but this is kept here for now
    //because it may be used in the future (was used in the past)
    //
    /// Lookup pattern that needs to be used to properly discern if
    /// `ScopeLookupPattern::Namespace/OnlyVar` should be used to search for the member associacted with
    /// this config member
    pub lookup_pat: ScopeLookupPattern,
    /// Members this member holds
    pub cfg_def_members: Vec<ImplMemberId>,
}

impl ConfigDefMember {
    pub fn new(
        name_id: InternedId,
        name_span: SourceSpan,
        impl_member_id: ImplMemberId,
        linked_member_id: MemberId,
        linked_member_type_id: Option<TypeId>,
        metadata: ConfigMemberMetadataKind,
        lookup_pat: ScopeLookupPattern,
        opt_assignments: Vec<ImplMemberId>,
        cfg_def_members: Vec<ImplMemberId>,
    ) -> ConfigDefMember {
        ConfigDefMember {
            name_id,
            impl_member_id,
            linked_member_id,
            linked_member_type_id,
            name_span,
            metadata,
            opt_assignments,
            lookup_pat,
            cfg_def_members,
        }
    }
}

// Maybe embed this into lookup instead?
#[derive(Debug, Clone)]
pub enum ConfigRootMetadataKind {
    Complex(SpannedContainer<TypeExpr>),
    /// Name span
    Override(SourceSpan),
}

// Oh my.
/// In `override`, it has the choice between directly interacting with a global namespace like "JAVA"
/// or with a user defined type.
pub enum OverrideConfigRootMetadataKind {
    Namespace(SpannedContainer<InternedId>),
    Type(SpannedContainer<TypeExpr>),
}

/// For allowing one config to hold different metadata depending on the context
#[derive(Debug, Clone)]
pub enum ConfigMemberMetadataKind {
    Complex(ComplexConfigMemberMetadata),
    Override(OverrideConfigMemberMetadata),
}

impl ConfigMemberMetadataKind {
    /// Returns `true` if complex variant, false otherwise
    /// `override` section
    pub fn is_complex(&self) -> bool {
        match self {
            ConfigMemberMetadataKind::Complex(_) => true,

            ConfigMemberMetadataKind::Override(_) => false,
        }
    }

    /// Returns `true` if override variant, false otherwise
    pub fn is_override(&self) -> bool {
        match self {
            ConfigMemberMetadataKind::Complex(_) => false,

            ConfigMemberMetadataKind::Override(_) => true,
        }
    }

    pub fn expect_complex(&self) -> &ComplexConfigMemberMetadata {
        match self {
            ConfigMemberMetadataKind::Complex(meta) => meta,
            _ => panic!("Expected `complex` metadata, found {:?}", self),
        }
    }

    pub fn complex(&self) -> Option<&ComplexConfigMemberMetadata> {
        match self {
            ConfigMemberMetadataKind::Complex(meta) => meta.into(),
            ConfigMemberMetadataKind::Override(_) => None,
        }
    }

    pub fn overrid(&self) -> Option<&OverrideConfigMemberMetadata> {
        match self {
            ConfigMemberMetadataKind::Override(meta) => todo!("Stamp."),
            ConfigMemberMetadataKind::Complex(_) => None,
        }
    }

    pub fn expect_override(&self) -> &OverrideConfigMemberMetadata {
        match self {
            ConfigMemberMetadataKind::Override(meta) => todo!("Stamp"),
            _ => panic!("Expected `override` metadata, found {:?}", self),
        }
    }

    // pub fn name_id(&self) -> InternedId {
    //     match self {
    //         ConfigMemberMetadataKind::Complex(meta) => {
    //             // If there exists a name id that means it was a declaration with a member of
    //             // some kind
    //             if let Some(name_id) = meta.name_id_opt {
    //                 name_id
    //
    //                 // If there exists no name id then it is an "override {}" block
    //                 // NOTE: Should probably just make this a specialized enum instead of
    //                 // heuristic decision making
    //             } else {
    //                 InternedId::new(intern::INTERNED_OVERRIDE)
    //             }
    //         }
    //         ConfigMemberMetadataKind::Override(meta) => todo!("STOP USING OVERRIDE"),
    //     }
    // }
}

//NOTE: UNUSED
/// `complex` scope `ConfigMember` specific metadata
#[derive(Debug, Clone)]
pub struct ComplexConfigMemberMetadata {}

impl ComplexConfigMemberMetadata {
    pub fn new() -> ComplexConfigMemberMetadata {
        ComplexConfigMemberMetadata {}
    }
}
/// `override` scope `ConfigMember` specific metadata
#[derive(Debug, Clone)]
pub struct OverrideConfigMemberMetadata {}
impl OverrideConfigMemberMetadata {
    pub fn new() -> OverrideConfigMemberMetadata {
        OverrideConfigMemberMetadata {}
    }
}

// Would be:
// Person {
//      .identifiers = "person" <--- This is a root opt
//      name {
//          .default_val = 3 <--- This is a member opt
//      }
// }
/// Represents options and their values assigned by the user at root
#[derive(Debug)]
pub struct OptionAssignmentRoot {
    /// `SymbolId` of the `ConfigDefRoot`
    pub parent_impl_id: ImplId,
    /// `ImplMemberId` of `self`
    pub member_id: ImplMemberId,
    // more like option_name_id
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    /// All values this option is attached to
    pub array_expr_id: ExprId,
}

impl OptionAssignmentRoot {
    pub fn new(
        parent_impl_id: ImplId,
        member_id: ImplMemberId,
        name_id: InternedId,
        name_span: SourceSpan,
        array_expr_id: ExprId,
    ) -> OptionAssignmentRoot {
        OptionAssignmentRoot {
            parent_impl_id,
            member_id,
            name_id,
            name_span,
            array_expr_id,
        }
    }
}

//TODO: Maybe make this OptionAssignment<SymbolId/MemberId> where parent member id is that
// Would be:
// Person {
//      .identifiers = "person" <--- This is a root opt
//      name {
//          .default_val = 3 <--- This is a member opt
//      }
// }
/// Represents options and their values assigned by the user inside of a member from the root, not
/// the root itself
#[derive(Debug)]
pub struct OptionAssignmentMember {
    /// `MemberId` of the `ConfigDefMember` it is derivative of
    pub parent_member_id: MemberId,
    /// `MemberId` of `self`
    pub impl_member_id: ImplMemberId,
    // more like option_name_id
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    pub array_expr_id: ExprId,
}

impl OptionAssignmentMember {
    pub fn new(
        parent_member_id: MemberId,
        impl_member_id: ImplMemberId,
        name_id: InternedId,
        name_span: SourceSpan,
        array_expr_id: ExprId,
    ) -> OptionAssignmentMember {
        OptionAssignmentMember {
            parent_member_id,
            impl_member_id,
            name_id,
            name_span,
            array_expr_id,
        }
    }
}

#[derive(Debug)]
pub struct MultiTypeAssignment {
    to_assign: Vec<TypeId>,
    // Maybe there will be `ExternTypeId` usage but not sure about that.
    /// Expects `SymbolKind::ExternType`
    assign_to: SymbolId,
}
