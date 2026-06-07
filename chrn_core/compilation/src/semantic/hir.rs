// Should likely split this eventually
use std::{collections::HashMap, fmt::Debug};

use chrn_utils::{
    fmter::{Formattable, Formatted},
    id_types::{
        AstId, ConfigId, ExprId, InternedId, MemberId, ModuleId, ScopeId, SpannedContainer,
        SymbolId, TypeId, ValueId,
    },
    source_map::source_span::SourceSpan,
};
use lang::{
    inner_args::InnerArgs,
    parser::ast::{BinaryOp, UnaryOp},
    types::{builtins::BuiltinType, type_constraints::TypeConstraintFlags},
};

use crate::{
    constraints::ArgConstraint,
    scopes::{AssociatedScopeKind, ScopeType},
};

// What is a drop? I am new to thinking i have never thought before what is RAII
// is that a gui framework
// Maybe named, global table, program table

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

#[derive(Debug)]
pub struct Symbol {
    pub name_id: InternedId,
    // pub name_span: Option<SourceSpan>,
    pub sym_id: SymbolId,
    //err span purposes
    pub ast_id: Option<AstId>,
    pub kind: SymbolKind,
    pub owner: ModuleId,
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
        owner: ModuleId,
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
            owner,
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
    Val(ValueId),
    /// Represents a reserved type id which allows for symbols such as unresolved variables to have
    /// a stable type id associated with it even if it isn't resolved yet. This is mainly intended
    /// to isolate this type of state inside of a kind of symbol, rather than polluting type-space.
    ReservedTypeSlot(TypeId),
    /// Represents a module symbol
    Module(ModuleId),
    Config(ConfigId),
    // Section(),
}

// Maybe, ConfigKind, UserConfigDef, IntrinsicConfigDef
#[derive(Debug)]
pub enum ConfigKind {
    Description(ConfigDescription),
    Def(ConfigDef),
}

/// Represent a configurataion description that must be followed,
#[derive(Debug)]
pub struct ConfigDescription {
    pub kind: ConfigDescriptionKind,
    pub option_desc: Vec<OptionDescripton>,
}

impl ConfigDescription {
    pub fn new(
        kind: ConfigDescriptionKind,
        option_desc: Vec<OptionDescripton>,
    ) -> ConfigDescription {
        ConfigDescription { kind, option_desc }
    }
}

/// Represents a configurations options, that are preloaded by the compiler as schemas to follow
#[derive(Debug)]
pub struct OptionDescripton {
    name_id: InternedId,
    constraints: Option<TypeConstraintFlags>,
}

#[derive(Debug)]
pub enum ConfigDescriptionKind {
    Struct,
    Enum,
    Field,
}

/// Intended to represent a configuration block environment that consumes options for a field.
#[derive(Debug)]
pub struct ConfigDef {
    /// Is a name id instead of symbol id since `NameResolver` merely registers names, with no
    /// knowledge of symbol specifics. A dependency system may be used in the future.
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    pub option_assignments: Vec<OptionAssignment>,
    pub inner_field_cfg: Vec<ConfigDef>,
}

impl ConfigDef {
    pub fn new(
        name_id: InternedId,
        name_span: SourceSpan,
        option_assignments: Vec<OptionAssignment>,
        inner_field_cfg: Vec<ConfigDef>,
    ) -> ConfigDef {
        ConfigDef {
            name_id,
            name_span,
            option_assignments,
            inner_field_cfg,
        }
    }
}

/// Represents options and their values assigned by the user
#[derive(Debug)]
pub struct OptionAssignment {
    // Own member id
    pub member_id: MemberId,
    // more like option_name_id
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    pub array: Vec<ExprId>,
}

impl OptionAssignment {
    pub fn new(
        member_id: MemberId,
        name_id: InternedId,
        name_span: SourceSpan,
        array: Vec<ExprId>,
    ) -> OptionAssignment {
        OptionAssignment {
            member_id,
            name_id,
            name_span,
            array,
        }
    }
}

/// An enum that represents any sort of inner member that could exist within a given parent symbol.
#[derive(Debug)]
pub enum MemberSymbolKind {
    Field(FieldRepre),
    Variant(VariantRepre),
    // FIX:
    /// **NOT ACTUALLY A MEMBER YET**
    Param(Param),
    FieldAssignment(OptionAssignment),
}

// Maybe this should be a member?
#[derive(Debug)]
pub struct Param {
    pub sym_id: SymbolId,
    //FIX: More like "FieldId"
    //
    pub type_id: TypeId,
    // Should become SpanId maybe.
    pub ast_id: AstId,
}

impl Param {
    pub fn new(sym_id: SymbolId, type_id: TypeId, ast_id: AstId) -> Param {
        Param {
            sym_id,
            type_id,
            ast_id,
        }
    }
}

#[derive(Debug)]
pub struct ResolvedExpr {
    // NOTE: Considering making a typesafe wrapper to unknown check explicitly
    pub type_id: TypeId,
    pub expr_hir: ExprHir,
    // May store these elsewhere depending on um...uh..unreachable!()
    pub inputs: Vec<ExprId>,
    // Should be one
    pub user: Option<ExprId>,
    pub span: SourceSpan,
    // This is not an option type even though `Value` as an `Option<Value>` because symbols are
    // already represented as unknown from `SymbolKind` and `Value` types already have the metadata
    // of their type and if they have a const value inside.
    pub val_id: ValueId,
}

impl ResolvedExpr {
    pub fn new(
        type_id: TypeId,
        expr_hir: ExprHir,
        val_id: ValueId,
        span: SourceSpan,
        inputs: Vec<ExprId>,
    ) -> ResolvedExpr {
        ResolvedExpr {
            type_id,
            expr_hir,
            inputs,
            span,
            user: None,
            val_id,
        }
    }
}

#[derive(Debug, Clone)]
pub enum ExprHir {
    Val(ValueId),
    Var(SymbolId),
    /// alias default(x) = [Equals(x = 3)]
    /// x = `SymbolId`, 5 = `ExprId`
    Default(ExprId, ExprId),
    // MemberAccess(),
    // Um
    /// Caller, arguments
    Call(ExprId, Vec<ExprId>),
    // MemberAccess(AbstractMemberAccess),
    Unary {
        op: UnaryOp,
        operand: ExprId,
    },
    BinaryExpr {
        lhs: ExprId,
        op: BinaryOp,
        rhs: ExprId,
    },
    Array(Vec<ExprId>),
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
    //Maybe SpannedInnerArgs is fine here
    pub glob_args: Vec<InnerArgs>,
}

impl StructDef {
    pub fn new(sym_id: SymbolId, name_span: SourceSpan, fields: Vec<MemberId>) -> StructDef {
        StructDef {
            sym_id,
            name_span,
            fields,
            glob_conds: Vec::new(),
            glob_args: Vec::new(),
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
    pub glob_args: Vec<InnerArgs>,
    pub glob_conds: Vec<ExprId>,
}

impl EnumDef {
    pub fn new(sym_id: SymbolId, name_span: SourceSpan, variants: Vec<MemberId>) -> EnumDef {
        EnumDef {
            sym_id,
            name_span,
            variants,
            glob_conds: Vec::new(),
            glob_args: Vec::new(),
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
    pub member_id: MemberId,
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    // Because enum types are nullable
    pub type_id: Option<TypeId>,
    // pub spanned_ty: Option<SpannedContainer<TypeId>>,
    // Points to variant within original Ast enum
    // Also, more so a "FieldId"
    pub ast_id: AstId,
    pub args: Vec<InnerArgs>,
    pub conds: Vec<ExprId>,
}

impl VariantRepre {
    pub fn new(
        member_id: MemberId,
        name_id: InternedId,
        name_span: SourceSpan,
        // spanned_ty: Option<SpannedContainer<TypeId>>,
        type_id: Option<TypeId>,
        ast_id: AstId,
    ) -> VariantRepre {
        VariantRepre {
            member_id,
            name_id,
            name_span,
            type_id,
            // spanned_ty,
            ast_id,
            conds: Vec::new(),
            args: Vec::new(),
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
    pub args: Vec<InnerArgs>,
}

impl TypeDef {
    pub fn new(sym_id: SymbolId, name_span: SourceSpan, type_id: TypeId) -> TypeDef {
        TypeDef {
            sym_id,
            name_span,
            type_id,
            conds: Vec::new(),
            args: Vec::new(),
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
        kind: FuncKind,
        is_callable: bool,
        type_constraints: TypeConstraintFlags,
        arg_constraints: Vec<ArgConstraint>,
        affects_type_constraint: bool,
        ret_type: TypeId,
    ) -> FuncDef {
        FuncDef {
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
    pub member_id: MemberId,
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    // To TypeDef
    pub type_id: TypeId,
    // Ast contained field id, maybe this should just be AstId
    pub ast_id: AstId,
    pub conds: Vec<ExprId>,
    pub args: Vec<InnerArgs>,
}

impl FieldRepre {
    pub fn new(
        member_id: MemberId,
        name_id: InternedId,
        name_span: SourceSpan,
        type_id: TypeId,
        ast_id: AstId,
    ) -> FieldRepre {
        FieldRepre {
            member_id,
            name_id,
            name_span,
            type_id,
            conds: Vec::new(),
            args: Vec::new(),
            ast_id,
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
    pub args: Vec<InnerArgs>,
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
            args: Vec::new(),
        }
    }
}

impl Formattable for AliasDef {
    fn to_fmt(&self) -> Formatted {
        Formatted::Alias
    }
}

//TEST:
pub(crate) enum PossibleMember {
    Type(TypeId),
    // Member(MemberId),
    Var(ValueId),
    Nothing,
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
