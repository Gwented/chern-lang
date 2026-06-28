// What is a drop? I am new to thinking i have never thought before what is RAII
// is that a gui framework
// Maybe named, global table, program table

use std::collections::HashMap;

use chrn_utils::{
    id_types::{ExprId, MemberId, ModuleId, ScopeId, SpannedContainer, TypeId, ValueId},
    source_map::source_span::SourceSpan,
};
use lang::{
    directives::Directive,
    fmter::{Formattable, Formatted},
    types::{builtins::BuiltinType, type_constraints::TypeConstraintFlags},
};

use crate::{
    constraints::ArgConstraint,
    lookup::scopes::{AssociatedScopeKind, ScopeType},
    script_compiler::ScriptCompiler,
    semantic::hir::hir_exprs::Param,
};

// This is kind of just a "concept" though
use chrn_utils::id_types::{AstId, ConfigId, DirectiveId, InternedId, SymbolId, VariableId};

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
    Module(ModuleId),
    /// Represents a config symbol
    Config(ConfigId),
    Directive(DirectiveId),
    // Section(),
}

impl SymbolKind {
    // This is getting obscure now...
    pub fn to_fmt(compiler: &ScriptCompiler, sym_id: SymbolId) -> Formatted {
        match &compiler.symbols[sym_id.id as usize].kind {
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
    Constrained(TypeConstraintFlags),
    /// Preserved stable handle so that anything defined before a type was defined can still point
    /// to the correct type which prevents duplicating different definitions.
    Deferred(TypeId),
    Unknown,
}

impl Type {
    /// The env can't be passed into to_fmt so
    pub fn to_fmt(compiler: &ScriptCompiler, type_id: TypeId) -> Formatted {
        match &compiler.types[type_id.id as usize].ty {
            Type::BuiltinType(builtin_type) => builtin_type.kind().to_fmt(),
            Type::Struct(struct_def) => struct_def.to_fmt(),
            Type::Enum(enum_def) => enum_def.to_fmt(),
            Type::Func(func_def) => func_def.to_fmt(),
            Type::Alias(alias_def) => alias_def.to_fmt(),
            Type::TypeDef(type_def) => type_def.to_fmt(),
            // This is the only issue since it's not a single Formatted.
            // The next obvious decision should be to do, "Formatted::NumericIntegerRanged", etc.,
            // where we have 4000 variants which
            Type::Constrained(flags) => Formatted::Constraints(*flags),
            Type::Deferred(inner) => Type::to_fmt(compiler, *inner),
            Type::Unknown => Formatted::Unknown,
        }
    }
}

#[derive(Debug)]
pub struct VarDef {
    pub sym_id: SymbolId,
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    // Same job as SymbolKind::ReservedTypeSlot
    pub state: VariableState,
    // Is option since forging a value id early is a lot of unneeded extra effort
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
/// been defined, and a variable that has actually been seen in some form (not resolved)
///
/// This abstraction was chosen to avoid having to directly update `VarDef` everytime a `Value` or
/// `ResolvedExpr` needed an update. If this enum were not used, that would mean every resolution
/// incremental buildup would need the tree to start at the original variable symbol, which makes
/// propagation much more complex, in comparison to the current route where it removes that concern
/// entirely from the `VarDef` itself.
#[derive(Debug)]
pub enum VariableState {
    ReservedTypeSlot(TypeId),
    Known(ValueId),
}

// TODO: Readiness for skipping during resolution
/// Intended to represent a configuration block environment that consumes options for a field.
#[derive(Debug)]
pub struct ConfigDef {
    /// Is a name id instead of symbol id since `NameResolver` merely registers names, with no
    /// knowledge of symbol specifics. A dependency system may be used in the future.
    pub name_id: InternedId,
    /// During name resolution, we can't actually lookup the symbol since it may or may not be
    /// registered, so it's Option since it actually is `None` at some point, and could remain
    /// `None` if in a later stage it doesn't have it's target symbol found.
    ///
    pub sym_id: Option<SymbolId>,
    pub name_span: SourceSpan,
    /// Expects `ConfigOptionAssignment`
    pub opt_assignments: Vec<MemberId>,
    pub inner_field_cfgs: Vec<ConfigId>,
}

impl ConfigDef {
    pub fn new(
        name_id: InternedId,
        name_span: SourceSpan,
        sym_id: Option<SymbolId>,
        option_assignments: Vec<MemberId>,
        inner_field_cfg: Vec<ConfigId>,
    ) -> ConfigDef {
        ConfigDef {
            name_id,
            name_span,
            sym_id,
            opt_assignments: option_assignments,
            inner_field_cfgs: inner_field_cfg,
        }
    }
}

/// Represents options and their values assigned by the user
#[derive(Debug)]
pub struct ConfigOptionAssignment {
    pub parent_sym_id: SymbolId,
    // Own member id
    pub member_id: MemberId,
    // more like option_name_id
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    pub array_expr_id: ExprId,
}

impl ConfigOptionAssignment {
    pub fn new(
        parent_sym_id: SymbolId,
        member_id: MemberId,
        name_id: InternedId,
        name_span: SourceSpan,
        array_expr_id: ExprId,
    ) -> ConfigOptionAssignment {
        ConfigOptionAssignment {
            parent_sym_id,
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
    Field(FieldRepre),
    Variant(VariantRepre),
    // FIX:
    // **NOT ACTUALLY A MEMBER YET**
    // Param(Param),
    OptionAssignment(ConfigOptionAssignment),
}

impl MemberSymbolKind {
    pub fn member_id(&self) -> MemberId {
        match self {
            MemberSymbolKind::Field(field_repre) => field_repre.member_id,
            MemberSymbolKind::Variant(variant_repre) => variant_repre.member_id,
            MemberSymbolKind::OptionAssignment(option_assignment) => option_assignment.member_id,
        }
    }

    pub fn parent_sym_id(&self) -> SymbolId {
        match self {
            MemberSymbolKind::Field(field_repre) => field_repre.parent_sym_id,
            MemberSymbolKind::Variant(variant_repre) => variant_repre.parent_sym_id,
            MemberSymbolKind::OptionAssignment(option_assignment) => {
                option_assignment.parent_sym_id
            }
        }
    }
}

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

#[derive(Debug)]
pub struct EnumDef {
    pub sym_id: SymbolId,
    pub name_span: SourceSpan,
    pub variants: Vec<MemberId>,
    pub glob_directives: Vec<SpannedContainer<DirectiveId>>,
    pub glob_conds: Vec<ExprId>,
}

impl EnumDef {
    pub fn new(sym_id: SymbolId, name_span: SourceSpan, variants: Vec<MemberId>) -> EnumDef {
        EnumDef {
            sym_id,
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

/// A HIR of enum variants created by script semantics
#[derive(Debug)]
pub struct VariantRepre {
    pub parent_sym_id: SymbolId,
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
        parent_sym_id: SymbolId,
        member_id: MemberId,
        name_id: InternedId,
        name_span: SourceSpan,
        // spanned_ty: Option<SpannedContainer<TypeId>>,
        type_id: Option<TypeId>,
        ast_id: AstId,
    ) -> VariantRepre {
        VariantRepre {
            parent_sym_id,
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
    pub name_span: SourceSpan,
    /// Represents the str in "var-> name: str"
    pub type_id: TypeId,
    pub conds: Vec<ExprId>,
    pub directives: Vec<SpannedContainer<DirectiveId>>,
}

impl TypeDef {
    pub fn new(sym_id: SymbolId, name_span: SourceSpan, type_id: TypeId) -> TypeDef {
        TypeDef {
            sym_id,
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
    pub type_constraints: TypeConstraintFlags,
    //TEST:
    pub arg_constraints: Vec<ArgConstraint>,
    pub ret_type: TypeId,
}

impl FuncDef {
    pub fn new(
        sym_id: SymbolId,
        kind: FuncKind,
        is_callable: bool,
        type_constraints: TypeConstraintFlags,
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
    pub parent_sym_id: SymbolId,
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
        parent_sym_id: SymbolId,
        member_id: MemberId,
        name_id: InternedId,
        name_span: SourceSpan,
        type_id: TypeId,
    ) -> FieldRepre {
        FieldRepre {
            parent_sym_id,
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
    pub ty_constraints: TypeConstraintFlags,
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
            ty_constraints: TypeConstraintFlags::runtime(),
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
