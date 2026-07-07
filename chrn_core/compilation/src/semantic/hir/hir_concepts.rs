// What is a drop? I am new to thinking i have never thought before what is RAII
// is that a gui framework
// Maybe named, global table, program table

use std::collections::HashMap;

use chrn_utils::{
    id_types::{ExprId, MemberId, ModuleId, ScopeId, SpannedContainer, TypeId, ValueId},
    loop_abort,
    source_map::source_span::SourceSpan,
};
use lang::{
    fmter::{Formattable, Formatted},
    types::{boundaries::TypeBoundaryFlags, builtins::BuiltinType},
};

use crate::{
    constraints::ArgConstraint,
    lookup::scopes::{AssociatedScopeKind, LookupPattern, ScopeType},
    script_compiler::ScriptCompiler,
    semantic::hir::hir_exprs::Param,
};

// This is kind of just a "concept" though
use chrn_utils::id_types::{AstId, ConfigRootId, DirectiveId, InternedId, SymbolId, VariableId};

#[derive(Debug)]
pub struct Table {
    // Can still change some to vec maybe
    pub(crate) ast_to_sym: HashMap<AstId, SymbolId>,
    //TEST:
    pub(crate) interned_to_sym: HashMap<InternedId, SymbolId>,
    // Maybe also to type
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

impl SymbolOrigin {
    pub fn try_as_module(&self) -> Option<ModuleId> {
        match self {
            SymbolOrigin::Module(mod_id) => Some(*mod_id),
            SymbolOrigin::Compiler => None,
        }
    }
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
    Module(ModuleId),
    /// Represents a config symbol
    Config(ConfigRootId),
    Directive(DirectiveId),
    // Section(),
}

impl SymbolKind {
    // This is getting obscure now...
    pub fn to_fmt(compiler: &ScriptCompiler, sym_id: SymbolId) -> Formatted {
        match &compiler.symbols[sym_id].kind {
            SymbolKind::Type(type_id) => Type::to_fmt(compiler, *type_id),
            SymbolKind::Variable(_) => Formatted::Variable,
            SymbolKind::Module(_) => Formatted::Module,
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
//NOTE: Should be in chrn_utils?
#[derive(Debug)]
pub enum Type {
    BuiltinType(BuiltinType),
    Struct(StructDef),
    Enum(EnumDef),
    Func(FuncDef),
    Alias(AliasDef),
    TypeDef(TypeDef),
    Constrained(TypeBoundaryFlags),
    /// Preserved stable handle so that anything defined before a type was defined can still point
    /// to the correct type which prevents duplicating different definitions.
    Deferred(TypeId),
    Unknown,
}

impl Type {
    //TEST:
    pub fn try_as_struct(&self) -> Option<&StructDef> {
        match self {
            Type::Struct(struct_def) => Some(struct_def),
            _ => None,
        }
    }

    /// The env can't be passed into to_fmt so
    pub fn to_fmt(compiler: &ScriptCompiler, mut type_id: TypeId) -> Formatted {
        for _ in 0..chrn_utils::MAX_LOOPS {
            // Could be an Option return where if is_none() look_abort! but probably doesn't matter.
            // At all.
            match &compiler.types[type_id].ty {
                Type::BuiltinType(builtin_type) => return builtin_type.kind().to_fmt(),
                Type::Struct(struct_def) => return struct_def.to_fmt(),
                Type::Enum(enum_def) => return enum_def.to_fmt(),
                Type::Func(func_def) => return func_def.to_fmt(),
                Type::Alias(alias_def) => return alias_def.to_fmt(),
                Type::TypeDef(type_def) => return type_def.to_fmt(),
                // This is the only issue since it's not a single Formatted.
                // The next obvious decision should be to do, "Formatted::NumericIntegerRanged", etc.,
                // where we have 4000 variants which
                Type::Constrained(flags) => return Formatted::Boundaries(*flags),
                Type::Unknown => return Formatted::Unknown,
                Type::Deferred(inner) => type_id = *inner,
            }
        }

        loop_abort!();
    }
}

#[derive(Debug)]
pub struct VarDef {
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
/// Intended to represent a configuration block environment that consumes options for a field.
#[derive(Debug)]
pub struct ConfigDefRoot {
    /// Is a name id instead of symbol id since `NameResolver` merely registers names, with no
    /// knowledge of symbol specifics. A dependency system may be used in the future.
    pub name_id: InternedId,
    // This is not a `SpannedContainer` because it may become an Option
    pub name_span: SourceSpan,
    /// ConfigId of `self`
    pub cfg_id: ConfigRootId,
    /// During name resolution, we can't actually lookup the symbol since it may or may not be
    /// registered, so it's Option since it actually is `None` at some point, and could remain
    /// `None` if in a later stage it doesn't have it's target symbol found.
    pub sym_id: Option<SymbolId>,
    /// Expects `ConfigOptionAssignment`
    pub opt_assignments: Vec<MemberId>,
    /// Lookup pattern that needs to be used to properly discern if
    /// `LookupPattern::Namespace/OnlyVar` should be used to search for the symbol associated with
    /// thie config
    pub lookup_pattern: LookupPattern,
    /// Expects `ConfigDefMember`
    pub cfg_members: Vec<MemberId>,
}

impl ConfigDefRoot {
    pub fn new(
        name_id: InternedId,
        name_span: SourceSpan,
        cfg_id: ConfigRootId,
        sym_id: Option<SymbolId>,
        lookup_pattern: LookupPattern,
        opt_assignments: Vec<MemberId>,
        cfg_members: Vec<MemberId>,
    ) -> ConfigDefRoot {
        ConfigDefRoot {
            name_id,
            name_span,
            cfg_id,
            lookup_pattern,
            sym_id,
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
    /// `MemberId` of the member symbol this is actually attached to
    pub member_id_origin: MemberId,
    /// Expects `ConfigOptionAssignment`
    pub opt_assignments: Vec<MemberId>,
    /// Lookup pattern that needs to be used to properly discern if
    /// `LookupPattern::Namespace/OnlyVar` should be used to search for the member associacted with
    /// this config member
    // Is this needed?
    pub lookup_pattern: LookupPattern,
    /// Members this member holds
    pub cfg_def_members: Vec<MemberId>,
}

impl ConfigDefMember {
    pub fn new(
        name_id: InternedId,
        name_span: SourceSpan,
        member_id: MemberId,
        member_id_origin: MemberId,
        opt_assignments: Vec<MemberId>,
        lookup_pattern: LookupPattern,
        cfg_def_members: Vec<MemberId>,
    ) -> ConfigDefMember {
        ConfigDefMember {
            member_id,
            member_id_origin,
            name_id,
            name_span,
            opt_assignments,
            lookup_pattern,
            cfg_def_members,
        }
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
    /// `SymbolId` of the `ConfigDefMember` it is derivative of
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
    Unknown(MemberId),
}

impl MemberSymbolKind {
    pub fn name_id(&self) -> Option<InternedId> {
        match self {
            MemberSymbolKind::Field(field_repre) => Some(field_repre.name_id),
            MemberSymbolKind::Variant(variant_repre) => Some(variant_repre.name_id),
            MemberSymbolKind::OptAssignmentRoot(opt_assignment_root) => {
                Some(opt_assignment_root.name_id)
            }
            MemberSymbolKind::OptAssignmentMember(opt_assignment_member) => {
                Some(opt_assignment_member.name_id)
            }
            MemberSymbolKind::ConfigDefMember(cfg_def_member) => Some(cfg_def_member.name_id),
            MemberSymbolKind::Unknown(_) => None,
        }
    }

    pub fn member_id(&self) -> MemberId {
        match self {
            MemberSymbolKind::Unknown(member_id) => *member_id,
            MemberSymbolKind::Field(field_repre) => field_repre.member_id,
            MemberSymbolKind::Variant(variant_repre) => variant_repre.member_id,
            MemberSymbolKind::OptAssignmentRoot(opt_assignment_root) => {
                opt_assignment_root.member_id
            }
            MemberSymbolKind::OptAssignmentMember(opt_assignment_member) => {
                opt_assignment_member.member_id
            }
            MemberSymbolKind::ConfigDefMember(cfg_def_member) => cfg_def_member.member_id,
        }
    }

    // TODO:
    pub fn local_parent_sym_id(&self) -> Option<SymbolId> {
        match self {
            MemberSymbolKind::Field(field_repre) => Some(field_repre.local_parent_sym_id),
            MemberSymbolKind::Variant(variant_repre) => Some(variant_repre.local_parent_sym_id),
            MemberSymbolKind::OptAssignmentRoot(option_assignment) => {
                // I don't think this applies here
                Some(option_assignment.parent_sym_id)
            }
            MemberSymbolKind::ConfigDefMember(_)
            | MemberSymbolKind::OptAssignmentMember(_)
            | MemberSymbolKind::Unknown(_) => None,
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
        kind: FuncKind,
        is_callable: bool,
        type_constraints: TypeBoundaryFlags,
        arg_constraints: Vec<ArgConstraint>,
        affects_type_constraint: bool,
        ret_type: TypeId,
    ) -> FuncDef {
        FuncDef {
            sym_id,
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
