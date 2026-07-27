// What is a drop? I am new to thinking i have never thought before what is RAII
// is that a gui framework
// Maybe named, global table, program table

use std::collections::HashMap;

use chrn_utils::{
    id_types::{ExprId, MemberId, ModuleId, ScopeId, SpannedContainer, TypeId, ValueId},
    intern, loop_abort,
    source_map::source_span::SourceSpan,
};
use lang::{
    fmter::{Formattable, Formatted},
    types::{
        boundaries::TypeBoundaryFlags,
        builtins::{BuiltinType, BuiltinTypeKind},
    },
};

use crate::{
    constraints::ArgConstraint,
    lookup::scopes::{AssociatedScopeKind, ScopeLookupPattern, ScopeType},
    script_compiler::ScriptCompiler,
    semantic::hir::hir_exprs::Param,
};

// This is kind of just a "concept" though
use chrn_utils::id_types::{AstId, ConfigRootId, DirectiveId, InternedId, SymbolId, VariableId};

// #[derive(Debug)]
// pub struct SectionInfo {
//     pub sections: [Option<SectionHir>; 5],
//     pub compilation_syms: SymbolId,
// }

// Who is this?
#[derive(Debug)]
pub struct Table {
    pub(crate) ast_to_sym: HashMap<AstId, SymbolId>,
    pub(crate) interned_to_sym: HashMap<InternedId, SymbolId>,
}

impl Table {
    pub fn new() -> Table {
        Table {
            ast_to_sym: HashMap::new(),
            interned_to_sym: HashMap::new(),
        }
    }
}

#[derive(Debug)]
pub enum SymbolOrigin {
    Module(ModuleId),
    Compiler,
}

#[derive(Debug)]
pub struct Symbol {
    pub name_id: InternedId,
    // pub name_span: Option<SourceSpan>,
    pub sym_id: SymbolId,
    //err span purposes
    pub ast_id: Option<AstId>,
    pub kind: SymbolKind,
    pub sym_origin: SymbolOrigin,
    pub scope_origin: ScopeType,
    // For something such as member access
    pub associated_scope: Option<AssociatedScopeKind>,
    pub is_priv: bool,
}

impl Symbol {
    pub fn new(
        // May couple dbg info but fine for now
        name_id: InternedId,
        // name_span: Option<SourceSpan>,
        sym_id: SymbolId,
        //dbgr
        // Maybe we can have an id enum instead with it possibility allowing for field types?
        ast_id: Option<AstId>,
        sym_origin: SymbolOrigin,
        is_priv: bool,
        associated_scope: Option<AssociatedScopeKind>,
        scope_origin: ScopeType,
        kind: SymbolKind,
    ) -> Symbol {
        Symbol {
            name_id,
            // name_span,
            sym_id,
            ast_id,
            kind,
            scope_origin,
            associated_scope,
            sym_origin,
            is_priv,
        }
    }
}

/// Maps to different notable symbols which index into their respectful vectors
#[derive(Debug, Clone, Copy)]
pub enum SymbolKind {
    /// Represents a type symbol
    Type(TypeId),
    /// Represents a variable symbol
    Variable(VariableId),
    // /// Represents a reserved type id which allows for symbols such as unresolved variables to have
    // /// a stable type id associated with it even if it isn't resolved yet. This is mainly intended
    // /// to isolate this type of state inside of a kind of symbol, rather than polluting type-space.
    // ReservedTypeSlot(TypeId),
    /// Represents a module symbol
    Namespace,
    /// Represents a config symbol
    Config(ConfigRootId),
    Directive(DirectiveId),
}

impl SymbolKind {
    // This is getting obscure now...
    pub fn to_fmt(compiler: &ScriptCompiler, sym_id: SymbolId) -> Formatted {
        let sym = &compiler.symbols[sym_id];
        match &sym.kind {
            SymbolKind::Type(type_id) => Type::to_fmt(compiler, *type_id),
            SymbolKind::Variable(_) => Formatted::Variable,
            SymbolKind::Namespace => match sym.associated_scope.expect("Is kind namespace") {
                AssociatedScopeKind::Module(_) => Formatted::Module,
                AssociatedScopeKind::Scope(_) => Formatted::Namespace,
            },
            SymbolKind::Config(_) => Formatted::Config,
            SymbolKind::Directive(_) => Formatted::Directive,
        }
    }
}

#[derive(Debug)]
pub struct TypeInfo {
    pub ty: Type,
    pub owner: ModuleId,
}

impl TypeInfo {
    pub fn new(ty: Type, owner: ModuleId) -> TypeInfo {
        TypeInfo { ty, owner }
    }
}

// Types are not given spans directly since it would over-complicate storing and add a net 12 byte
// increase to all spans. Also, type spanning is entity symbol dependent anyways so it's likely the
// better choice.
//NOTE: Should be in lang?
#[derive(Debug)]
pub enum Type {
    BuiltinTypeInfo(BuiltinTypeInfo),
    Struct(StructDef),
    Enum(EnumDef),
    Func(FuncDef),
    Alias(AliasDef),
    TypeDef(TypeDef),
    Boundaries(TypeBoundaryFlags),
    /// Preserved stable handle so that anything defined before a type was defined can still point
    /// to the correct type which prevents duplicating different definitions.
    Deferred(TypeId),
    Unknown,
}

/// Required metadata for compiler built-in types
#[derive(Debug)]
pub struct BuiltinTypeInfo {
    pub sym_id: SymbolId,
    pub ty: BuiltinType,
}

impl BuiltinTypeInfo {
    pub fn new(sym_id: SymbolId, ty: BuiltinType) -> BuiltinTypeInfo {
        BuiltinTypeInfo { sym_id, ty }
    }
}

impl Type {
    //TEST: Usually uses associated functions
    pub fn kind(compiler: &ScriptCompiler, mut type_id: TypeId) -> TypeKind {
        for _ in 0..chrn_utils::MAX_LOOPS {
            match &compiler.types[type_id].ty {
                Type::BuiltinTypeInfo(builtin_ty) => {
                    return TypeKind::BuiltinType(builtin_ty.ty.kind());
                }
                Type::Struct(_) => return TypeKind::Struct,
                Type::Enum(_) => return TypeKind::Enum,
                Type::Func(_) => return TypeKind::Func,
                Type::Alias(_) => return TypeKind::Alias,
                Type::TypeDef(_) => return TypeKind::TypeDef,
                // This is the only issue since it's not a single Formatted.
                // The next obvious decision should be to do, "Formatted::NumericIntegerRanged", etc.,
                // where we have 4000 variants which
                Type::Boundaries(_) => return TypeKind::Boundaries,
                Type::Unknown => return TypeKind::Unknown,
                Type::Deferred(inner) => type_id = *inner,
            }
        }
        loop_abort!();
    }

    pub fn boundaries(compiler: &ScriptCompiler, mut type_id: TypeId) -> Option<TypeBoundaryFlags> {
        for _ in 0..chrn_utils::MAX_LOOPS {
            match &compiler.types[type_id].ty {
                Type::BuiltinTypeInfo(builtin_ty) => {
                    return Some(builtin_ty.ty.kind().boundaries());
                }
                // This is the only issue since it's not a single Formatted.
                // The next obvious decision should be to do, "Formatted::NumericIntegerRanged", etc.,
                Type::Struct(_)
                | Type::Enum(_)
                | Type::Func(_)
                | Type::Alias(_)
                | Type::Unknown => return None,
                // where we have 4000 variants which
                Type::Boundaries(boundaries) => return Some(*boundaries),
                Type::TypeDef(type_def) => type_id = type_def.type_id,
                Type::Deferred(inner) => type_id = *inner,
            }
        }
        loop_abort!()
    }

    /// The env can't be passed into to_fmt so
    pub fn to_fmt(compiler: &ScriptCompiler, mut type_id: TypeId) -> Formatted {
        for _ in 0..chrn_utils::MAX_LOOPS {
            // Could be an Option return where if is_none() look_abort! but probably doesn't matter.
            // At all.
            match &compiler.types[type_id].ty {
                Type::BuiltinTypeInfo(builtin_type) => return builtin_type.ty.kind().to_fmt(),
                Type::Struct(struct_def) => return struct_def.to_fmt(),
                Type::Enum(enum_def) => return enum_def.to_fmt(),
                Type::Func(func_def) => return func_def.to_fmt(),
                Type::Alias(alias_def) => return alias_def.to_fmt(),
                Type::TypeDef(type_def) => return type_def.to_fmt(),
                // This is the only issue since it's not a single Formatted.
                // The next obvious decision should be to do, "Formatted::NumericIntegerRanged", etc.,
                // where we have 4000 variants which
                Type::Boundaries(flags) => return Formatted::Boundaries(*flags),
                Type::Unknown => return Formatted::Unknown,
                Type::Deferred(inner) => type_id = *inner,
            }
        }
        loop_abort!()
    }
}

// WE LOST
/// Flat variation of `Type`
pub enum TypeKind {
    BuiltinType(BuiltinTypeKind),
    Struct,
    TypeDef,
    Boundaries,
    Enum,
    Func,
    Alias,
    Unknown,
}

#[derive(Debug)]
pub struct VarDef {
    /// `SymbolId` of `self`
    pub sym_id: SymbolId,
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    // Same job as SymbolKind::ReservedTypeSlot
    pub state: VariableState,
}

impl VarDef {
    pub fn new(
        sym_id: SymbolId,
        name_id: InternedId,
        name_span: SourceSpan,
        state: VariableState,
    ) -> VarDef {
        VarDef {
            sym_id,
            name_id,
            name_span,
            state,
        }
    }
}

/// This enum is used inside of variables to create the separation between a variable that has only
/// been registered, and a variable that has actually been seen in some form (not resolved)
///
/// This abstraction was chosen to avoid having to directly update `VarDef` everytime a `ValueInfo` or
/// `ResolvedExpr` needed an update. If this enum were not used, that would mean every resolution
/// incremental buildup would need the tree to start at the original variable symbol, which makes
/// propagation much more complex, in comparison to the current route where it removes that concern
/// entirely from the `VarDef` itself.
// I cannot read
#[derive(Debug)]
pub enum VariableState {
    ReservedTypeSlot(TypeId),
    Known(ValueId),
}

// TODO: Readiness for skipping during resolution
/// Intended to represent a config block environment that consumes options for a field.
#[derive(Debug)]
pub struct ConfigDefRoot {
    /// `SymbolId` of `self`
    pub parent_sym_id: SymbolId,
    //NOTE: ConfigDefRoot cannot be made from a keyword so it always has a name_id.
    //
    /// Is a name id instead of symbol id since `NameResolver` merely registers names, with no
    /// knowledge of symbol specifics. A dependency system may be used in the future.
    pub name_id: InternedId,
    // This is not a `SpannedContainer` because it may become an Option
    pub name_span: SourceSpan,
    /// ConfigId of `self`
    pub cfg_root_id: ConfigRootId,
    /// During name resolution, we can't actually lookup the symbol since it may or may not be
    /// registered, so it's Option since it actually is `None` at some point, and could remain
    /// `None` if in a later stage it doesn't have it's target symbol found.
    pub linked_sym_id: Option<SymbolId>,
    /// Expects `OptionAssignmentRoot`
    pub opt_assignments: Vec<MemberId>,
    /// Lookup pattern that needs to be used to properly discern if
    /// `ScopeLookupPattern::Namespace/OnlyVar` should be used to search for the symbol associated with
    /// thie config
    pub lookup_pattern: ScopeLookupPattern,
    /// Expects `ConfigDefMember`
    pub cfg_members: Vec<MemberId>,
}

impl ConfigDefRoot {
    pub fn new(
        parent_sym_id: SymbolId,
        name_id: InternedId,
        name_span: SourceSpan,
        cfg_root_id: ConfigRootId,
        linked_sym_id: Option<SymbolId>,
        lookup_pattern: ScopeLookupPattern,
        opt_assignments: Vec<MemberId>,
        cfg_members: Vec<MemberId>,
    ) -> ConfigDefRoot {
        ConfigDefRoot {
            parent_sym_id,
            name_id,
            name_span,
            cfg_root_id,
            lookup_pattern,
            linked_sym_id,
            opt_assignments,
            cfg_members,
        }
    }
}

/// The member inside of a `ConfigDef` or `ConfigDefMember` which is the same structure,
/// but with ties to a `MemberSymbol` instead of a `Symbol`
#[derive(Debug)]
pub struct ConfigDefMember {
    /// Is a name id instead of symbol id since `NameResolver` merely registers names, with no
    /// knowledge of symbol specifics. A dependency system may be used in the future.
    pub name_id: InternedId,
    // This is not a `SpannedContainer` because it may become an Option
    pub name_span: SourceSpan,
    /// `MemberId` of `self`
    pub member_id: MemberId,
    /// `MemberId` of the member symbol this is attached to
    pub linked_member_id: MemberId,
    /// Expects `OptionAssignmentMember`
    pub opt_assignments: Vec<MemberId>,
    // These configs are supposed to be usable by override too so maybe this becomes an enum where
    // it exposes metadata depending on override or not.
    pub metadata: ConfigMemberMetadataKind,
    /// Lookup pattern that needs to be used to properly discern if
    /// `ScopeLookupPattern::Namespace/OnlyVar` should be used to search for the member associacted with
    /// this config member
    pub lookup_pattern: ScopeLookupPattern,
    /// Members this member holds
    pub cfg_def_members: Vec<MemberId>,
}

impl ConfigDefMember {
    pub fn new(
        name_id: InternedId,
        name_span: SourceSpan,
        member_id: MemberId,
        linked_member_id: MemberId,
        metadata: ConfigMemberMetadataKind,
        opt_assignments: Vec<MemberId>,
        lookup_pattern: ScopeLookupPattern,
        cfg_def_members: Vec<MemberId>,
    ) -> ConfigDefMember {
        ConfigDefMember {
            name_id,
            member_id,
            linked_member_id,
            name_span,
            metadata,
            opt_assignments,
            lookup_pattern,
            cfg_def_members,
        }
    }
}

// Maybe embed this into lookup instead?
// #[derive(Debug, Clone)]
// pub enum ConfigRootMetadataKind {
//     Override,
// }

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
    pub parent_sym_id: SymbolId,
    /// `MemberId` of `self`
    pub member_id: MemberId,
    // more like option_name_id
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    /// All values this option is attached to
    pub array_expr_id: ExprId,
}

impl OptionAssignmentRoot {
    pub fn new(
        parent_sym_id: SymbolId,
        member_id: MemberId,
        name_id: InternedId,
        name_span: SourceSpan,
        array_expr_id: ExprId,
    ) -> OptionAssignmentRoot {
        OptionAssignmentRoot {
            parent_sym_id,
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
    pub member_id: MemberId,
    // more like option_name_id
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    pub array_expr_id: ExprId,
}

impl OptionAssignmentMember {
    pub fn new(
        parent_member_id: MemberId,
        member_id: MemberId,
        name_id: InternedId,
        name_span: SourceSpan,
        array_expr_id: ExprId,
    ) -> OptionAssignmentMember {
        OptionAssignmentMember {
            parent_member_id,
            member_id,
            name_id,
            name_span,
            array_expr_id,
        }
    }
}

/// An enum that represents any sort of inner member that could exist within a given parent symbol.
#[derive(Debug)]
pub enum MemberSymbolKind {
    /// Represents `struct` fields
    Field(FieldRepre),
    /// Represents `enum` fields
    Variant(VariantRepre),
    // FIX:
    // **NOT ACTUALLY A MEMBER YET**
    // Param(Param),
    /// `ConfigDefMember`
    ConfigDefMember(ConfigDefMember),
    /// Root specific option assignment
    OptAssignmentRoot(OptionAssignmentRoot),
    /// Member specific option assignment
    OptAssignmentMember(OptionAssignmentMember),
    /// Member that has reserved a slot but not yet defined
    Unknown {
        sp_name_id: SpannedContainer<InternedId>,
        reserved_member_id: MemberId,
    },
}

impl MemberSymbolKind {
    /// Attempts to get boundaries out of member.
    ///
    /// Only field and variant members are considered to have underlying types.
    // Should that be the case though?
    pub fn boundaries(compiler: &ScriptCompiler, member_id: MemberId) -> Option<TypeBoundaryFlags> {
        let type_id_opt = match &compiler.members[member_id] {
            MemberSymbolKind::Field(field_repre) => Some(field_repre.type_id),
            MemberSymbolKind::Variant(variant_repre) => variant_repre.type_id,
            MemberSymbolKind::ConfigDefMember(_)
            | MemberSymbolKind::OptAssignmentRoot(_)
            | MemberSymbolKind::OptAssignmentMember(_)
            | MemberSymbolKind::Unknown { .. } => None,
        };

        if let Some(type_id) = type_id_opt {
            Type::boundaries(compiler, type_id)
        } else {
            None
        }
    }

    /// Returns `None` if the type is unknown
    pub fn name_id(&self) -> InternedId {
        match self {
            MemberSymbolKind::Field(field_repre) => field_repre.name_id,
            MemberSymbolKind::Variant(variant_repre) => variant_repre.name_id,
            MemberSymbolKind::OptAssignmentRoot(opt_assignment_root) => opt_assignment_root.name_id,
            MemberSymbolKind::OptAssignmentMember(opt_assignment_member) => {
                opt_assignment_member.name_id
            }
            MemberSymbolKind::ConfigDefMember(cfg_def_member) => cfg_def_member.name_id,
            MemberSymbolKind::Unknown { sp_name_id, .. } => sp_name_id.inner,
        }
    }

    pub fn member_id(&self) -> MemberId {
        match self {
            MemberSymbolKind::Field(field_repre) => field_repre.member_id,
            MemberSymbolKind::Variant(variant_repre) => variant_repre.member_id,
            MemberSymbolKind::OptAssignmentRoot(opt_assignment_root) => {
                opt_assignment_root.member_id
            }
            MemberSymbolKind::OptAssignmentMember(opt_assignment_member) => {
                opt_assignment_member.member_id
            }
            MemberSymbolKind::ConfigDefMember(cfg_def_member) => cfg_def_member.member_id,
            MemberSymbolKind::Unknown {
                reserved_member_id, ..
            } => *reserved_member_id,
        }
    }

    pub fn is_unknown(&self) -> bool {
        match self {
            MemberSymbolKind::Unknown { .. } => true,
            _ => false,
        }
    }

    // TODO:
    /// A local parent is the parent this member symbol was declared in, rather than it's actual
    /// parent symbol. For example, if we have "Person { state: State }" The local parent of `state`
    /// is `Person`, but the actual parent would be considered the declaration of `State` itself.
    pub fn local_parent_sym_id(&self) -> Option<SymbolId> {
        match self {
            MemberSymbolKind::Field(field_repre) => Some(field_repre.local_parent_sym_id),
            MemberSymbolKind::Variant(variant_repre) => Some(variant_repre.local_parent_sym_id),
            MemberSymbolKind::ConfigDefMember(_)
            | MemberSymbolKind::OptAssignmentRoot(_)
            | MemberSymbolKind::OptAssignmentMember(_)
            | MemberSymbolKind::Unknown { .. } => None,
        }
    }
}

/// HIR representation of the language `struct` type
#[derive(Debug)]
pub struct StructDef {
    pub sym_id: SymbolId,
    pub name_span: SourceSpan,
    pub fields: Vec<MemberId>,
    pub glob_conds: Vec<ExprId>,
    pub glob_directives: Vec<SpannedContainer<DirectiveId>>,
}

impl StructDef {
    pub fn new(sym_id: SymbolId, name_span: SourceSpan, fields: Vec<MemberId>) -> StructDef {
        StructDef {
            sym_id,
            name_span,
            fields,
            glob_conds: Vec::new(),
            glob_directives: Vec::new(),
        }
    }
}

impl Formattable for StructDef {
    fn to_fmt(&self) -> Formatted {
        Formatted::Struct
    }
}

/// HIR representation of the language `enum` type
#[derive(Debug)]
pub struct EnumDef {
    pub sym_id: SymbolId,
    // Is not present because the symbol also holds the name id which would be duplicated an id. May
    // change to where it includes it anyways.
    // pub name_id: InternedId,
    pub name_span: SourceSpan,
    pub variants: Vec<MemberId>,
    pub glob_directives: Vec<SpannedContainer<DirectiveId>>,
    pub glob_conds: Vec<ExprId>,
}

impl EnumDef {
    pub fn new(
        sym_id: SymbolId,
        // name_id: InternedId,
        name_span: SourceSpan,
        variants: Vec<MemberId>,
    ) -> EnumDef {
        EnumDef {
            sym_id,
            // name_id,
            name_span,
            variants,
            glob_conds: Vec::new(),
            glob_directives: Vec::new(),
        }
    }
}

impl Formattable for EnumDef {
    fn to_fmt(&self) -> Formatted {
        Formatted::Enum
    }
}

/// HIR representation of the language `variant` type
#[derive(Debug)]
pub struct VariantRepre {
    // TODO: Need to maybe bundle this with the Option TypeId since they both mean the same thing in
    // that there is some other type declared, but at the same time it could be a built in type,
    // which doesn't have a declaration location but does have a type id so these are still
    // disconnected. Most importantly, would it be bad to leave this as NOT an option where the
    // caller can choose to care if the local parent is the actual parent?
    // Maybe this shouldn't
    // exist at all and the caller should have to explicitly look for the origin with the same
    // compiler method.
    /// SymbolId of the type in which this variant was locally declared in, not the symbol
    /// id of the original declaration location of the type associated with this variant.
    /// So if struct `Person` had a field of `State`, `State` would consider `Person` it's local
    /// parent, but the actual declaration location of `State` as a struct/enum itself would be in
    /// an entirely different place
    pub local_parent_sym_id: SymbolId,
    /// MemberId of `self`
    pub member_id: MemberId,
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    // Because enum types are nullable
    pub type_id: Option<TypeId>,
    // pub spanned_ty: Option<SpannedContainer<TypeId>>,
    // Points to variant within original Ast enum
    // Also, more so a "FieldId"
    pub ast_id: AstId,
    pub directives: Vec<SpannedContainer<DirectiveId>>,
    pub conds: Vec<ExprId>,
}

impl VariantRepre {
    pub fn new(
        local_parent_sym_id: SymbolId,
        member_id: MemberId,
        name_id: InternedId,
        name_span: SourceSpan,
        // spanned_ty: Option<SpannedContainer<TypeId>>,
        type_id: Option<TypeId>,
        ast_id: AstId,
    ) -> VariantRepre {
        VariantRepre {
            local_parent_sym_id,
            member_id,
            name_id,
            name_span,
            type_id,
            // spanned_ty,
            ast_id,
            conds: Vec::new(),
            directives: Vec::new(),
        }
    }
}

/// Typedefs are: "var-> name: str" meaning the typedef type has types so it has a type id
#[derive(Debug)]
pub struct TypeDef {
    pub sym_id: SymbolId,
    // The padding fills this to 72 bytes anyways so this does nothing but give convenience and
    // reduce lookup
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    /// Represents the str in "var-> name: str"
    pub type_id: TypeId,
    pub conds: Vec<ExprId>,
    pub directives: Vec<SpannedContainer<DirectiveId>>,
}

impl TypeDef {
    pub fn new(
        sym_id: SymbolId,
        name_id: InternedId,
        name_span: SourceSpan,
        type_id: TypeId,
    ) -> TypeDef {
        TypeDef {
            sym_id,
            name_id,
            name_span,
            type_id,
            conds: Vec::new(),
            directives: Vec::new(),
        }
    }
}

impl Formattable for TypeDef {
    fn to_fmt(&self) -> Formatted {
        Formatted::TypeDef
    }
}

#[derive(Debug)]
pub struct FuncDef {
    pub name_id: InternedId,
    pub sym_id: SymbolId,
    pub kind: FuncKind,
    // May be separate structure
    pub is_callable: bool,
    /// Given:
    /// x: i32 \[IsEmpty\]
    /// IsEmpty's usage in this example directly depends on the type of self.
    /// But given "Log(x)", it would not be dependent on self, meaning it should be ignored in
    /// regards to
    pub affects_type_constraint: bool,
    //TEST:
    pub type_constraints: TypeBoundaryFlags,
    //TEST:
    pub arg_constraints: Vec<ArgConstraint>,
    pub ret_type: TypeId,
}

impl FuncDef {
    pub fn new(
        sym_id: SymbolId,
        name_id: InternedId,
        kind: FuncKind,
        is_callable: bool,
        type_constraints: TypeBoundaryFlags,
        arg_constraints: Vec<ArgConstraint>,
        affects_type_constraint: bool,
        ret_type: TypeId,
    ) -> FuncDef {
        FuncDef {
            sym_id,
            name_id,
            kind,
            is_callable,
            affects_type_constraint,
            type_constraints,
            arg_constraints,
            ret_type,
        }
    }
}

impl Formattable for FuncDef {
    fn to_fmt(&self) -> Formatted {
        Formatted::Func
    }
}

#[derive(Debug)]
pub struct FieldRepre {
    /// SymbolId of the type in which this field was locally declared in, not the symbol
    /// id of the original declaration location of the type associated with this field.
    /// So if struct `Person` had a field of `State`, `State` would consider `Person` it's local
    /// parent, but the actual declaration location of `State` as a struct/enum itself would be in
    /// an entirely different place
    pub local_parent_sym_id: SymbolId,
    /// MemberId of `self`
    pub member_id: MemberId,
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    // To TypeDef
    pub type_id: TypeId,
    // Ast contained field id, maybe this should just be AstId
    // pub ast_id: AstId,
    pub conds: Vec<ExprId>,
    pub directives: Vec<SpannedContainer<DirectiveId>>,
}

impl FieldRepre {
    pub fn new(
        local_parent_sym_id: SymbolId,
        member_id: MemberId,
        name_id: InternedId,
        name_span: SourceSpan,
        type_id: TypeId,
    ) -> FieldRepre {
        FieldRepre {
            local_parent_sym_id,
            member_id,
            name_id,
            name_span,
            type_id,
            conds: Vec::new(),
            directives: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct AliasDef {
    pub sym_id: SymbolId,
    pub name_span: SourceSpan,
    pub params: Vec<Param>,
    pub ty_constraints: TypeBoundaryFlags,
    pub arg_constraints: Vec<ArgConstraint>,
    pub local_scope_id: ScopeId,
    pub directives: Vec<SpannedContainer<DirectiveId>>,
    pub conds: Vec<ExprId>,
}

impl AliasDef {
    pub fn new(
        sym_id: SymbolId,
        name_span: SourceSpan,
        params: Vec<Param>,
        arg_constraints: Vec<ArgConstraint>,
        local_scope_id: ScopeId,
    ) -> AliasDef {
        AliasDef {
            sym_id,
            name_span,
            params,
            ty_constraints: TypeBoundaryFlags::runtime(),
            arg_constraints,
            local_scope_id,
            conds: Vec::new(),
            directives: Vec::new(),
        }
    }
}

impl Formattable for AliasDef {
    fn to_fmt(&self) -> Formatted {
        Formatted::Alias
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FuncKind {
    IsEmpty,
    IsWhitespace,
    Contains,
    Range,
    StartsW,
    EndsW,
    Equals,
}

impl Formattable for FuncKind {
    fn to_fmt(&self) -> Formatted {
        match self {
            FuncKind::Contains => Formatted::FuncContains,
            FuncKind::IsWhitespace => Formatted::IsWhitespace,
            FuncKind::Range => Formatted::FuncRange,
            FuncKind::StartsW => Formatted::FuncStartsW,
            FuncKind::EndsW => Formatted::FuncEndsW,
            FuncKind::Equals => Formatted::FuncEquals,
            FuncKind::IsEmpty => Formatted::IsEmpty,
        }
    }
}
