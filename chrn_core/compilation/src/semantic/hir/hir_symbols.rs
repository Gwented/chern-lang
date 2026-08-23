use chrn_utils::{
    id_types::{
        AstId, DirectiveId, ExprId, InternedId, MemberId, ModuleId, ScopeId, SymbolId, TypeId,
        ValueId, VariableId,
    },
    source_map::source_span::SourceSpan,
    utils::containers::SpannedContainer,
};
use lang::{
    chrn_classifier::{ChrnClassifiable, ChrnClassifier},
    types::boundaries::TypeBoundaryFlags,
};

use crate::{
    constraints::ArgConstraint,
    lookup::scopes::scopes_concepts::{AssociatedScopeKind, ScopeType},
    script_compiler::ScriptCompiler,
    semantic::hir::{hir_concepts::Type, hir_exprs::Param},
};

#[derive(Debug, Clone, Copy)]
pub enum SymbolOrigin {
    Module(ModuleId),
    Compiler,
}

#[derive(Debug)]
pub struct Symbol {
    pub name_id: InternedId,
    // pub name_span: Option<SourceSpan>,
    /// `SymbolId` of `self`
    pub sym_id: SymbolId,
    //err span purposes
    /// `AstId` of `self`
    pub ast_id: Option<AstId>,
    pub kind: SymbolKind,
    pub sym_origin: SymbolOrigin,
    pub scope_origin: ScopeType,
    // For something such as member access
    pub associated_scope: Option<AssociatedScopeKind>,
    pub is_priv: bool,
}

impl Symbol {
    pub const fn new(
        // May couple dbg info but fine for now
        name_id: InternedId,
        // name_span: Option<SourceSpan>,
        sym_id: SymbolId,
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
    /// Represents a namespace of any kind. Can currently be either a module symbol or plain
    /// namespace.
    Namespace,
    Directive(DirectiveId),
    ExternType,
}

impl SymbolKind {
    // This is getting obscure now...
    pub fn to_fmt(compiler: &ScriptCompiler, sym_id: SymbolId) -> ChrnClassifier {
        let sym = &compiler.symbols[sym_id];
        match &sym.kind {
            SymbolKind::Type(type_id) => Type::to_fmt(&compiler.types, *type_id),
            SymbolKind::Variable(_) => ChrnClassifier::Variable,
            SymbolKind::Namespace => match sym.associated_scope.expect("Is kind namespace") {
                AssociatedScopeKind::Module(_) => ChrnClassifier::Module,
                AssociatedScopeKind::Scope(_) => ChrnClassifier::Namespace,
            },
            SymbolKind::Directive(_) => ChrnClassifier::Directive,
            SymbolKind::ExternType => ChrnClassifier::ExternType,
        }
    }
    pub fn to_flat(&self) -> SymbolKindFlat {
        match self {
            SymbolKind::Type(_) => SymbolKindFlat::Type,
            SymbolKind::Variable(_) => SymbolKindFlat::Variable,
            SymbolKind::Namespace => SymbolKindFlat::Namespace,
            SymbolKind::Directive(_) => SymbolKindFlat::Directive,
            SymbolKind::ExternType => SymbolKindFlat::ExternType,
        }
    }
}

/// Flat representation of `SymbolKind`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKindFlat {
    Type,
    Variable,
    Namespace,
    Directive,
    ExternType,
}

impl SymbolKindFlat {
    // Need bit suffix to avoid namespace collision
    pub const TYPE_BITS: u16 = 1 << 1;
    pub const VARIABLE_BITS: u16 = 1 << 2;
    pub const NAMESPACE_BITS: u16 = 1 << 3;
    pub const DIRECTIVE_BITS: u16 = 1 << 4;
    pub const EXTERN_TYPE_BITS: u16 = 1 << 5;

    pub fn to_bits(self) -> u16 {
        match self {
            SymbolKindFlat::Type => Self::TYPE_BITS,
            SymbolKindFlat::Variable => Self::VARIABLE_BITS,
            SymbolKindFlat::Namespace => Self::NAMESPACE_BITS,
            SymbolKindFlat::Directive => Self::DIRECTIVE_BITS,
            SymbolKindFlat::ExternType => Self::EXTERN_TYPE_BITS,
        }
    }
}

#[derive(Debug)]
pub struct VarDef {
    /// `SymbolId` of `self`
    pub sym_id: SymbolId,
    pub name_id: InternedId,
    pub meta: VariableMetadata,
    // Same job as SymbolKind::ReservedTypeSlot
    pub state: VariableState,
}

impl VarDef {
    pub fn new(
        sym_id: SymbolId,
        name_id: InternedId,
        meta: VariableMetadata,
        state: VariableState,
    ) -> VarDef {
        VarDef {
            sym_id,
            name_id,
            meta,
            state,
        }
    }
}

#[derive(Debug)]
pub enum VariableMetadata {
    User(SourceSpan),
    Generated,
}

impl VariableMetadata {
    pub fn expect_user(&self) -> SourceSpan {
        match self {
            VariableMetadata::User(span) => *span,
            VariableMetadata::Generated => {
                panic!("Expected `VariableMetadata::User`, found `{self:?}`")
            }
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

/// An enum that represents any sort of inner member that could exist within a given parent symbol.
#[derive(Debug)]
pub enum MemberSymbolKind {
    /// Represents `struct` fields
    Field(FieldRepre),
    /// Represents `enum` fields
    Variant(VariantRepre),
    //// Member that has reserved a slot but not yet defined
    // Unknown {
    //     sp_name_id: SpannedContainer<InternedId>,
    //     reserved_member_id: MemberId,
    // },
}

impl MemberSymbolKind {
    /// Attempts to get boundaries out of member.
    ///
    /// Only field and variant members are considered to have underlying types.
    // Should that be the case though?
    pub fn boundaries(compiler: &ScriptCompiler, member_id: MemberId) -> Option<TypeBoundaryFlags> {
        let type_id_opt = match &compiler.sym_members[member_id] {
            MemberSymbolKind::Field(field_repre) => Some(field_repre.type_id),
            MemberSymbolKind::Variant(variant_repre) => variant_repre.type_id,
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
        }
    }

    /// Returns `None` if the type is unknown
    pub fn name_span(&self) -> SourceSpan {
        match self {
            MemberSymbolKind::Field(field_repre) => field_repre.name_span,
            MemberSymbolKind::Variant(variant_repre) => variant_repre.name_span,
        }
    }

    pub fn member_id(&self) -> MemberId {
        match self {
            MemberSymbolKind::Field(field_repre) => field_repre.member_id,
            MemberSymbolKind::Variant(variant_repre) => variant_repre.member_id,
        }
    }

    // TODO:
    /// A local parent is the parent this member symbol was declared in, rather than it's actual
    /// parent symbol. For example, if we have "Person { state: State }" The local parent of `state`
    /// is `Person`, but the actual parent would be considered the declaration of `State` itself.
    pub fn local_parent_sym_id(&self) -> SymbolId {
        match self {
            MemberSymbolKind::Field(field_repre) => field_repre.local_parent_sym_id,
            MemberSymbolKind::Variant(variant_repre) => variant_repre.local_parent_sym_id,
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

impl ChrnClassifiable for StructDef {
    fn to_classified(&self) -> ChrnClassifier {
        ChrnClassifier::Struct
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

impl ChrnClassifiable for EnumDef {
    fn to_classified(&self) -> ChrnClassifier {
        ChrnClassifier::Enum
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

impl ChrnClassifiable for TypeDef {
    fn to_classified(&self) -> ChrnClassifier {
        ChrnClassifier::TypeDef
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
    // I don't remember adding this to be entirely, completely, honest.
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

impl ChrnClassifiable for FuncDef {
    fn to_classified(&self) -> ChrnClassifier {
        ChrnClassifier::Func
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

impl ChrnClassifiable for AliasDef {
    fn to_classified(&self) -> ChrnClassifier {
        ChrnClassifier::Alias
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

impl ChrnClassifiable for FuncKind {
    fn to_classified(&self) -> ChrnClassifier {
        match self {
            FuncKind::Contains => ChrnClassifier::FuncContains,
            FuncKind::IsWhitespace => ChrnClassifier::IsWhitespace,
            FuncKind::Range => ChrnClassifier::FuncRange,
            FuncKind::StartsW => ChrnClassifier::FuncStartsW,
            FuncKind::EndsW => ChrnClassifier::FuncEndsW,
            FuncKind::Equals => ChrnClassifier::FuncEquals,
            FuncKind::IsEmpty => ChrnClassifier::IsEmpty,
        }
    }
}
