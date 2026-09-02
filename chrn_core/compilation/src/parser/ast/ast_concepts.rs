use chrn_utils::{
    arena::Arena,
    id_types::{AstId, InternedId},
    source_map::source_span::SourceSpan,
    utils::containers::SpannedContainer,
};
use lang::{
    chrn_classifier::{ChrnClassifiable, ChrnClassifier},
    types::boundaries::TypeBoundaryFlags,
};

use crate::{
    lookup::scopes::scopes_concepts::{ScopeLookupPattern, ScopeType},
    parser::ast::{
        ast_exprs::{PathSegment, SpannedExpr, TypeExpr},
        ast_stmts::AbstractStmt,
    },
    semantic::hir::hir_impls::ConfigRootMetadataKind,
};

// Maybe this type of thing should go into an ast_concepts module?
/// Ast.
#[derive(Debug)]
pub struct AstInfo {
    /// Array that holds all 5 `chrn` sections.
    /// order: `neutral`, `var`, `nest`, `complex`, `override`
    pub sections: [Option<AbstractSection>; 4],
    pub items: Arena<Item, AstId>,
}

impl AstInfo {
    pub fn new() -> AstInfo {
        AstInfo {
            sections: [None, None, None, None],
            items: Arena::new(),
        }
    }

    pub fn with_capacity(items_cap: usize) -> AstInfo {
        AstInfo {
            sections: [None, None, None, None],
            items: Arena::with_capacity(items_cap),
        }
    }

    pub fn push_item(&mut self, kind: SectionKind, item: Item) {
        let ast_id = AstId::new(self.items.len() as u32);
        self.items.push(item);

        let sect = if let Some(sect) = &mut self.sections[kind as usize] {
            sect
        } else {
            self.push_sect(kind);
            &mut self.sections[kind as usize].as_mut().expect("Just created")
        };

        sect.nodes.push(ast_id);
    }

    pub fn push_sect(&mut self, kind: SectionKind) {
        self.sections[kind as usize] = AbstractSection::new(Vec::new(), kind).into();
    }

    pub fn sections(&self) -> &[Option<AbstractSection>] {
        &self.sections
    }

    pub fn items(&self) -> &Vec<Item> {
        // Oh my
        &self.items.items
    }

    pub fn get_decl(&self, ast_id: AstId) -> &AbstractDecl {
        match &self.items[ast_id] {
            Item::Decl(abs_decl) => abs_decl,
            _ => unreachable!(),
        }
    }

    pub fn get_impl(&self, ast_id: AstId) -> &AbstractImpl {
        match &self.items[ast_id] {
            Item::Impl(abs_impl) => abs_impl,
            Item::Decl(_) => unreachable!(),
        }
    }

    pub fn get_typedef(&self, ast_id: AstId) -> &AbstractTypeDef {
        match &self.items[ast_id] {
            item => match item {
                Item::Decl(abs_decl) => match abs_decl {
                    AbstractDecl::TypeDef(abs_typedef) => abs_typedef,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub fn get_struct(&self, ast_id: AstId) -> &AbstractStruct {
        match &self.items[ast_id] {
            item => match item {
                Item::Decl(abs_decl) => match abs_decl {
                    AbstractDecl::Struct(abs_struct) => abs_struct,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub fn get_var(&self, ast_id: AstId) -> &AbstractVar {
        match &self.items[ast_id] {
            item => match item {
                Item::Decl(abs_decl) => match abs_decl {
                    AbstractDecl::Var(abs_var) => abs_var,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    /// The only actual configs that can be accessed are config roots from the ast so this
    /// guaranteed to output a config root.
    pub fn get_cfg_root(&self, ast_id: AstId) -> &AbstractConfig {
        match &self.items[ast_id] {
            item => match item {
                Item::Impl(abs_impl) => match abs_impl {
                    AbstractImpl::Config(abs_cfg) => abs_cfg,
                },
                _ => unreachable!(),
            },
        }
    }

    pub fn get_enum(&self, ast_id: AstId) -> &AbstractEnum {
        match &self.items[ast_id] {
            item => match item {
                Item::Decl(abs_decl) => match abs_decl {
                    AbstractDecl::Enum(abs_enum) => abs_enum,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub fn get_alias(&self, ast_id: AstId) -> &AbstractAlias {
        match &self.items[ast_id] {
            item => match item {
                Item::Decl(abs_decl) => match abs_decl {
                    AbstractDecl::Alias(abs_alias) => abs_alias,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            },
        }
    }

    pub fn get_name_span(&self, ast_id: AstId) -> SourceSpan {
        match &self.items[ast_id] {
            item => match item {
                Item::Decl(abs_decl) => match abs_decl {
                    AbstractDecl::TypeDef(abs_typedef) => abs_typedef.name_span,
                    AbstractDecl::Struct(abs_struct) => abs_struct.name_span,
                    AbstractDecl::Enum(abs_enum) => abs_enum.name_span,
                    AbstractDecl::Alias(abs_alias) => abs_alias.name_span,
                    AbstractDecl::Var(abs_var) => abs_var.name_span,
                },
                _ => unreachable!(),
            },
        }
    }
}

#[derive(Debug)]
pub enum Item {
    // Should these have spans? Do we REALLY want      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    // No, we do not.
    Decl(AbstractDecl),
    Impl(AbstractImpl),
}

// Better name...
#[derive(Debug)]
pub enum AbstractDecl {
    TypeDef(AbstractTypeDef),
    Struct(AbstractStruct),
    Enum(AbstractEnum),
    Alias(AbstractAlias),
    Var(AbstractVar),
}

impl AbstractDecl {
    pub fn span(&self) -> SourceSpan {
        match self {
            AbstractDecl::TypeDef(abs_typedef) => abs_typedef.name_span,
            AbstractDecl::Struct(abs_struct) => abs_struct.name_span,
            AbstractDecl::Enum(abs_enum) => abs_enum.name_span,
            AbstractDecl::Alias(abs_alias) => abs_alias.name_span,
            AbstractDecl::Var(abs_var) => abs_var.name_span,
        }
    }
}

#[derive(Debug)]
pub enum AbstractImpl {
    Config(AbstractConfig),
}

#[derive(Debug)]
pub struct AbstractSection {
    pub(crate) nodes: Vec<AstId>,
    pub(crate) kind: SectionKind,
}

impl AbstractSection {
    pub fn new(nodes: Vec<AstId>, kind: SectionKind) -> AbstractSection {
        AbstractSection { nodes, kind }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Neutral = 0,
    Var = 1,
    Nest = 2,
    Complex = 3,
}

impl SectionKind {
    pub fn to_scope_type(self) -> ScopeType {
        match self {
            SectionKind::Neutral => ScopeType::Neutral,
            SectionKind::Var => ScopeType::Var,
            SectionKind::Nest => ScopeType::Nest,
            SectionKind::Complex => ScopeType::Complex,
        }
    }
}

impl ChrnClassifiable for SectionKind {
    fn to_classified(&self) -> ChrnClassifier {
        match self {
            SectionKind::Neutral => ChrnClassifier::SectNeutral,
            SectionKind::Var => ChrnClassifier::SectVar,
            SectionKind::Nest => ChrnClassifier::SectNest,
            SectionKind::Complex => ChrnClassifier::SectComplex,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mult,
    Div,
    Greater,
    Less,
    GreaterOrEq,
    LessOrEq,
    Mod,
    And,
    Or,
    EqTo,
    NotEq,
    BitOr,
    BitAnd,
    BitRightShift,
    BitLeftShift,
    BitXor,
}

impl BinaryOp {
    pub fn is_arithmetic_op(&self) -> bool {
        match self {
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mult
            | BinaryOp::Div
            | BinaryOp::BitOr
            | BinaryOp::BitAnd
            | BinaryOp::BitRightShift
            | BinaryOp::BitLeftShift
            | BinaryOp::BitXor
            | BinaryOp::Mod => true,
            BinaryOp::Less
            | BinaryOp::GreaterOrEq
            | BinaryOp::Greater
            | BinaryOp::LessOrEq
            | BinaryOp::And
            | BinaryOp::Or
            | BinaryOp::EqTo
            | BinaryOp::NotEq => false,
        }
    }

    pub fn is_bool_op(&self) -> bool {
        match self {
            BinaryOp::Less
            | BinaryOp::GreaterOrEq
            | BinaryOp::LessOrEq
            | BinaryOp::And
            | BinaryOp::Or
            | BinaryOp::EqTo
            | BinaryOp::NotEq => true,
            BinaryOp::Add
            | BinaryOp::Sub
            | BinaryOp::Mult
            | BinaryOp::Div
            | BinaryOp::BitOr
            | BinaryOp::BitAnd
            | BinaryOp::BitRightShift
            | BinaryOp::BitLeftShift
            | BinaryOp::BitXor
            | BinaryOp::Greater
            | BinaryOp::Mod => false,
        }
    }
}

impl ChrnClassifiable for BinaryOp {
    fn to_classified(&self) -> ChrnClassifier {
        match self {
            BinaryOp::Add => ChrnClassifier::OpAdd,
            BinaryOp::Sub => ChrnClassifier::Hyphen,
            BinaryOp::Mult => ChrnClassifier::OpMult,
            BinaryOp::Div => ChrnClassifier::OpDivide,
            BinaryOp::Greater => ChrnClassifier::OpGreater,
            BinaryOp::Less => ChrnClassifier::OpLess,
            BinaryOp::GreaterOrEq => ChrnClassifier::OpGreaterOrEq,
            BinaryOp::LessOrEq => ChrnClassifier::OpLessOrEq,
            BinaryOp::Mod => ChrnClassifier::OpMod,
            BinaryOp::And => ChrnClassifier::OpAnd,
            BinaryOp::Or => ChrnClassifier::OpOr,
            BinaryOp::EqTo => ChrnClassifier::OpEqualTo,
            BinaryOp::NotEq => ChrnClassifier::OpNotEq,
            BinaryOp::BitOr => ChrnClassifier::OpBitOr,
            BinaryOp::BitAnd => ChrnClassifier::OpBitAnd,
            BinaryOp::BitRightShift => ChrnClassifier::OpBitRightShift,
            BinaryOp::BitLeftShift => ChrnClassifier::OpBitLeftShift,
            BinaryOp::BitXor => ChrnClassifier::OpBitXor,
        }
    }
}

// Maybe type inference could pick up on the fact that if a definition has a condition, and that
// condition is applied to only a particular bit size, then it should be that bit size
#[derive(Debug)]
pub struct AbstractVar {
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    pub spanned_expr: SpannedExpr,
    pub is_priv: bool,
}

impl AbstractVar {
    pub fn new(
        name_id: InternedId,
        name_span: SourceSpan,
        spanned_expr: SpannedExpr,
        is_priv: bool,
    ) -> AbstractVar {
        AbstractVar {
            name_id,
            name_span,
            spanned_expr,
            is_priv,
        }
    }
}

#[derive(Debug)]
pub struct AbstractDirective {
    pub sp_name_id: SpannedContainer<InternedId>,
}

impl AbstractDirective {
    pub fn new(sp_name_id: SpannedContainer<InternedId>) -> AbstractDirective {
        AbstractDirective { sp_name_id }
    }
}

#[derive(Debug)]
pub struct AbstractTypeDef {
    /// Identifier of `self`
    pub name_id: InternedId,
    /// Span for identifer of `self`
    pub name_span: SourceSpan,
    pub sp_ty_expr: SpannedContainer<TypeExpr>,
    pub is_priv: bool,
    pub conds: Vec<SpannedExpr>,
    pub directives: Vec<AbstractDirective>,
}

impl AbstractTypeDef {
    pub fn new(
        name_id: InternedId,
        name_span: SourceSpan,
        sp_ty_expr: SpannedContainer<TypeExpr>,
        directives: Vec<AbstractDirective>,
        is_priv: bool,
        conds: Vec<SpannedExpr>,
    ) -> AbstractTypeDef {
        AbstractTypeDef {
            name_id,
            name_span,
            is_priv,
            sp_ty_expr,
            directives,
            conds,
        }
    }
}

#[derive(Debug)]
pub struct AbstractStruct {
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    pub glob_conds: Vec<SpannedExpr>,
    pub glob_directives: Vec<AbstractDirective>,
    pub fields: Vec<AbstractTypeDef>,
    pub is_priv: bool,
}

impl AbstractStruct {
    pub fn new(
        name_id: InternedId,
        name_span: SourceSpan,
        glob_conds: Vec<SpannedExpr>,
        glob_directives: Vec<AbstractDirective>,
        fields: Vec<AbstractTypeDef>,
        is_priv: bool,
    ) -> AbstractStruct {
        AbstractStruct {
            name_id,
            name_span,
            glob_directives,
            glob_conds,
            fields,
            is_priv,
        }
    }
}

#[derive(Debug)]
pub struct AbstractEnum {
    // Would be SymbolId in symbol table anyways
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    pub variants: Vec<AbstractVariant>,
    pub glob_conds: Vec<SpannedExpr>,
    pub glob_directives: Vec<AbstractDirective>,
    pub is_priv: bool,
    // pub(crate) visibility: Visibility,
}

impl AbstractEnum {
    pub fn new(
        name_id: InternedId,
        name_span: SourceSpan,
        variants: Vec<AbstractVariant>,
        glob_conds: Vec<SpannedExpr>,
        glob_directives: Vec<AbstractDirective>,
        is_priv: bool,
    ) -> AbstractEnum {
        AbstractEnum {
            name_id,
            name_span,
            variants,
            glob_conds,
            glob_directives,
            is_priv,
        }
    }
}

// Hold that thought
#[derive(Debug)]
pub struct AbstractVariant {
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    // I think this is right?
    pub sp_ty_expr: Option<SpannedContainer<TypeExpr>>,
    pub directives: Vec<AbstractDirective>,
    pub conds: Vec<SpannedExpr>,
}

impl AbstractVariant {
    pub fn new(
        name_id: InternedId,
        name_span: SourceSpan,
        // I think this is right?
        sp_ty_expr: Option<SpannedContainer<TypeExpr>>,
        conds: Vec<SpannedExpr>,
        directives: Vec<AbstractDirective>,
    ) -> AbstractVariant {
        AbstractVariant {
            name_id,
            name_span,
            sp_ty_expr,
            directives,
            conds,
        }
    }
}

#[derive(Debug)]
pub struct AbstractFunc {
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    pub params: Vec<SpannedExpr>,
}

impl AbstractFunc {
    pub fn new(
        name_id: InternedId,
        name_span: SourceSpan,
        params: Vec<SpannedExpr>,
    ) -> AbstractFunc {
        AbstractFunc {
            name_id,
            name_span,
            params,
        }
    }
}

#[derive(Debug)]
pub struct AbstractFuncDecl {
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    pub params: Vec<AbstractParam>,
}

impl AbstractFuncDecl {
    pub fn new(
        name_id: InternedId,
        name_span: SourceSpan,
        params: Vec<AbstractParam>,
    ) -> AbstractFuncDecl {
        AbstractFuncDecl {
            name_id,
            name_span,
            params,
        }
    }
}
// Give parser distinct member, root configs?

#[derive(Debug)]
pub struct AbstractConfig {
    // In regards to "var->" defined variables, I think just allowing for, "var::inner" would be the
    // best in regards to accessing and changing fields
    // Could be a "Outer.a { }" where it is defining it's fields config specifically
    /// Name of assumed struct/enum type to configure
    // pub name_id: InternedId,
    /// Span assocaited with name to configure
    // pub name_span: SourceSpan,
    //// Config specific to the origin of this metadata. ONLY `ConfigMember` can have this.
    pub kind: AbstractConfigKind,
    /// Configuration options for the current parent to apply
    pub abs_stmts: Vec<AbstractStmt>,
    /// `ScopeType` that should be looked within for the given identifier
    /// Can only be `ScopeLookupPattern::OnlyVar/NamespaceOnly`
    pub lookup_pat: ScopeLookupPattern,
    /// Configuration for inner fields to define recursively
    pub cfg_members: Vec<AbstractConfig>,
}

impl AbstractConfig {
    pub fn new(
        kind: AbstractConfigKind,
        lookup_pat: ScopeLookupPattern,
        abs_stmts: Vec<AbstractStmt>,
        cfg_members: Vec<AbstractConfig>,
    ) -> AbstractConfig {
        AbstractConfig {
            kind,
            lookup_pat,
            abs_stmts,
            cfg_members,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigRootKindFlat {
    Complex,
    Override,
}

#[derive(Debug, Clone)]
pub enum AbstractConfigKind {
    /// Name attached to root
    Root(Vec<SpannedContainer<PathSegment>>, ConfigRootMetadataKind),
    /// :(
    Member(SpannedContainer<InternedId>, AstConfigMemberMetadataKind),
}

/// For allowing one config to hold different metadata depending on the context
#[derive(Debug, Clone)]
pub enum AstConfigMemberMetadataKind {
    Complex(AstComplexConfigMetadata),
    Override(AstOverrideConfigMetadata),
}

impl AstConfigMemberMetadataKind {
    /// Returns `true` if complex variant, false otherwise
    /// `override` section
    pub fn is_complex(&self) -> bool {
        match self {
            AstConfigMemberMetadataKind::Complex(_) => true,
            AstConfigMemberMetadataKind::Override(_) => false,
        }
    }

    /// Returns `true` if override variant, false otherwise
    pub fn is_override(&self) -> bool {
        match self {
            AstConfigMemberMetadataKind::Override(_) => true,
            AstConfigMemberMetadataKind::Complex(_) => false,
        }
    }

    pub fn expect_complex(&self) -> &AstComplexConfigMetadata {
        match self {
            AstConfigMemberMetadataKind::Complex(meta) => meta,
            _ => panic!("Expected `complex` metadata, found {:?}", self),
        }
    }

    pub fn expect_override(&self) -> &AstOverrideConfigMetadata {
        match self {
            AstConfigMemberMetadataKind::Override(meta) => meta,
            _ => panic!("Expected `override` metadata, found {:?}", self),
        }
    }
}

//NOTE: UNUSED
/// `complex` scope `ConfigMember` specific metadata
#[derive(Debug, Clone)]
pub struct AstComplexConfigMetadata {}

impl AstComplexConfigMetadata {
    pub const fn new() -> Self {
        Self {}
    }
}

/// `complex` scope `ConfigMember` specific metadata
#[derive(Debug, Clone)]
pub struct AstOverrideConfigMetadata {}

impl AstOverrideConfigMetadata {
    pub const fn new() -> Self {
        Self {}
    }
}

#[derive(Debug)]
pub struct AbstractParam {
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    pub sp_ty_expr: SpannedContainer<TypeExpr>,
}

impl AbstractParam {
    pub fn new(
        name_id: InternedId,
        name_span: SourceSpan,
        sp_ty_expr: SpannedContainer<TypeExpr>,
    ) -> AbstractParam {
        AbstractParam {
            name_id,
            name_span,
            sp_ty_expr,
        }
    }
}

#[derive(Debug)]
pub struct AbstractFieldDecl {
    pub name_id: InternedId,
    pub fields: Vec<SpannedExpr>,
}

// impl AbstractFieldDecl {
//     pub(crate) fn new(base: Box<Expr>, fields: Vec<Expr>) -> AbstractFieldDecl {
//         AbstractFieldDecl { base, fields }
//     }
// }

#[derive(Debug)]
pub struct AbstractAlias {
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    pub params: Vec<AbstractParam>,
    pub conds: Vec<SpannedExpr>,
    pub directives: Vec<AbstractDirective>,
    pub is_priv: bool,
}

impl AbstractAlias {
    pub fn new(
        name_id: InternedId,
        name_span: SourceSpan,
        params: Vec<AbstractParam>,
        conds: Vec<SpannedExpr>,
        directives: Vec<AbstractDirective>,
        is_priv: bool,
    ) -> AbstractAlias {
        AbstractAlias {
            name_id,
            name_span,
            params,
            conds,
            directives,
            is_priv,
        }
    }
}

#[derive(Debug)]
pub struct AbstractMemberAccess {
    pub base: Box<SpannedExpr>,
    pub field: InternedId,
}

impl AbstractMemberAccess {
    pub fn new(base: Box<SpannedExpr>, field: InternedId) -> AbstractMemberAccess {
        AbstractMemberAccess { base, field }
    }
}

#[derive(Debug)]
pub struct Unary {
    pub op: UnaryOp,
    pub spanned_expr: Box<SpannedExpr>,
}

impl Unary {
    pub fn new(op: UnaryOp, spanned_expr: Box<SpannedExpr>) -> Unary {
        Unary { op, spanned_expr }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum UnaryOp {
    Not,
    Negate,
    BitNot,
}

impl UnaryOp {
    pub fn boundaries(&self) -> TypeBoundaryFlags {
        match self {
            UnaryOp::Not => TypeBoundaryFlags::BOOL,
            UnaryOp::Negate => TypeBoundaryFlags::NUMERIC,
            UnaryOp::BitNot => TypeBoundaryFlags::INTEGER,
        }
    }
}

impl ChrnClassifiable for UnaryOp {
    fn to_classified(&self) -> ChrnClassifier {
        match self {
            UnaryOp::Not => ChrnClassifier::ExclamationPoint,
            UnaryOp::Negate => ChrnClassifier::Hyphen,
            UnaryOp::BitNot => ChrnClassifier::OpBitNot,
        }
    }
}
