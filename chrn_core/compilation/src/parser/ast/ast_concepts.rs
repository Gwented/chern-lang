use chrn_utils::{
    arena::Arena,
    id_types::{AstId, InternedId, SpannedContainer},
    source_map::source_span::SourceSpan,
};
use lang::{
    fmter::{Formattable, Formatted},
    types::boundaries::TypeBoundaryFlags,
};

use crate::{
    lookup::scopes::{ScopeLookupPattern, ScopeType},
    parser::ast::ast_exprs::{PathSegment, SpannedExpr, TypeExpr},
    semantic::hir::hir_impls::ConfigMemberMetadataKind,
};

// Maybe this type of thing should go into an ast_concepts module?
/// Ast.
#[derive(Debug)]
pub struct AstInfo {
    /// Array that holds all 5 `chrn` sections.
    pub sections: [Option<AbstractSection>; 5],
    pub items: Arena<Item, AstId>,
}

impl AstInfo {
    pub fn new() -> AstInfo {
        AstInfo {
            sections: [None, None, None, None, None],
            items: Arena::new(),
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

    // pub(super) fn get_func(&self, ast_id: AstId) -> &AbstractFunc {
    //     match &self.items[ast_id ] {
    //         item => match item {
    //             Item::Func(abs_struct) => abs_struct,
    //             _ => unreachable!(),
    //         },
    //     }
    // }

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
    Override = 3,
    Complex = 4,
}

impl SectionKind {
    pub fn to_scope_type(self) -> ScopeType {
        match self {
            SectionKind::Neutral => ScopeType::Neutral,
            SectionKind::Var => ScopeType::Var,
            SectionKind::Nest => ScopeType::Nest,
            SectionKind::Override => ScopeType::Override,
            SectionKind::Complex => ScopeType::Complex,
        }
    }
}

impl Formattable for SectionKind {
    fn to_fmt(&self) -> Formatted {
        match self {
            SectionKind::Neutral => Formatted::SectNeutral,
            SectionKind::Var => Formatted::SectVar,
            SectionKind::Nest => Formatted::SectNest,
            SectionKind::Override => Formatted::SectOverride,
            SectionKind::Complex => Formatted::SectComplex,
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

impl Formattable for BinaryOp {
    fn to_fmt(&self) -> Formatted {
        match self {
            BinaryOp::Add => Formatted::OpAdd,
            BinaryOp::Sub => Formatted::Hyphen,
            BinaryOp::Mult => Formatted::OpMult,
            BinaryOp::Div => Formatted::OpDivide,
            BinaryOp::Greater => Formatted::OpGreater,
            BinaryOp::Less => Formatted::OpLess,
            BinaryOp::GreaterOrEq => Formatted::OpGreaterOrEq,
            BinaryOp::LessOrEq => Formatted::OpLessOrEq,
            BinaryOp::Mod => Formatted::OpMod,
            BinaryOp::And => Formatted::OpAnd,
            BinaryOp::Or => Formatted::OpOr,
            BinaryOp::EqTo => Formatted::OpEqualTo,
            BinaryOp::NotEq => Formatted::OpNotEq,
            BinaryOp::BitOr => Formatted::OpBitOr,
            BinaryOp::BitAnd => Formatted::OpBitAnd,
            BinaryOp::BitRightShift => Formatted::OpBitRightShift,
            BinaryOp::BitLeftShift => Formatted::OpBitLeftShift,
            BinaryOp::BitXor => Formatted::OpBitXor,
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
    // inner:
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
        // visibility: Visibility,
    ) -> AbstractStruct {
        AbstractStruct {
            name_id,
            name_span,
            glob_directives,
            glob_conds,
            fields,
            is_priv, // visibility,
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
    // common: AbstractConfigCommon,
    // In regards to "var->" defined variables, I think just allowing for, "var.inner" would be the
    // best in regards to accessing and changing fields
    // Could be a "Outer.a { }" where it is defining it's fields config specifically
    /// Name of assumed struct/enum type to configure
    // pub name_id: InternedId,
    /// Span assocaited with name to configure
    // pub name_span: SourceSpan,
    //// Config specific to the origin of this metadata. ONLY `ConfigDefMember` can have this.
    pub kind: AbstractConfigKind,
    /// Configuration options for the current parent to apply
    pub opt_assignments: Vec<AbstractOptionAssignment>,
    /// `ScopeType` that should be looked within for the given identifier
    /// Can only be `ScopeLookupPattern::OnlyVar/NamespaceOnly`
    pub lookup_pat: ScopeLookupPattern,
    /// Configuration for inner fields to define recursively
    pub cfg_members: Vec<AbstractConfig>,
}

//TEST:
/// Semantic!
pub enum ConfigMemberNameKind {
    Interned(InternedId),
    Override,
}

impl AbstractConfig {
    pub fn new(
        kind: AbstractConfigKind,
        lookup_pat: ScopeLookupPattern,
        opt_assignments: Vec<AbstractOptionAssignment>,
        cfg_members: Vec<AbstractConfig>,
    ) -> AbstractConfig {
        AbstractConfig {
            kind,
            lookup_pat,
            opt_assignments,
            cfg_members,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigRootKind {
    Complex,
    Override,
}

#[derive(Debug, Clone)]
pub enum AbstractConfigKind {
    /// Name attached to root
    Root(Vec<SpannedContainer<PathSegment>>),
    /// :(
    Member(SpannedContainer<InternedId>, ConfigMemberMetadataKind),
}

//TEST: Might need this if these are genuinely distinctly different types if an origin is
//accounted for beep
// #[derive(Debug)]
// pub struct AbstractConfigInner {
//     // In regards to "var->" defined variables, I think just allowing for, "var.inner" would be the
//     // best in regards to accessing and changing fields
//     // Could be a "Outer.a { }" where it is defining it's fields config specifically
//     /// Name of assumed member to configure
//     pub name_id: InternedId,
//     /// Span assocaited with name to configure
//     pub name_span: SourceSpan,
//     /// Configuration options for the current member to apply
//     pub opt_assignments: Vec<AbstractOptionAssignment>,
//     /// Configuration for inner fields to define recursively
//     pub inner_field_cfg: Vec<AbstractConfigInner>,
// }
//
// impl AbstractConfigInner {
//     pub fn new(
//         name_id: InternedId,
//         name_span: SourceSpan,
//         opt_assignments: Vec<AbstractOptionAssignment>,
//         inner_field_cfg: Vec<AbstractConfigInner>,
//     ) -> AbstractConfigInner {
//         AbstractConfigInner {
//             name_id,
//             name_span,
//             opt_assignments,
//             inner_field_cfg,
//         }
//     }
// }
//
// // Would not be ScopeType since the other scope types would be marked as unreachable to where it
// // would seeming be non-productive.
// // Or maybe not?
// // pub enum RootConfigOrigin {
// //      Var,
// //      Nest
// // }

/// outer { .inner = Expr }
#[derive(Debug)]
pub struct AbstractOptionAssignment {
    /// Name of structural type to configure
    pub name_id: InternedId,
    pub name_span: SourceSpan,
    /// Must be an `ArrayExpr`
    pub array_expr: SpannedExpr,
}

impl AbstractOptionAssignment {
    pub fn new(
        name_id: InternedId,
        name_span: SourceSpan,
        array_expr: SpannedExpr,
    ) -> AbstractOptionAssignment {
        AbstractOptionAssignment {
            name_id,
            name_span,
            array_expr,
        }
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

impl Formattable for UnaryOp {
    fn to_fmt(&self) -> Formatted {
        match self {
            UnaryOp::Not => Formatted::ExclamationPoint,
            UnaryOp::Negate => Formatted::Hyphen,
            UnaryOp::BitNot => Formatted::OpBitNot,
        }
    }
}

/// TypeExpr: "TypeExpr, TypeExpr, TypeExpr = TypeExpr"
/// or: "i32, u32 = java::int"
#[derive(Debug, Clone)]
pub struct AbstractMultiAssign {
    // Need better terms
    /// -> (i32, u32, u8) = java::int
    to_assign: Vec<SpannedContainer<TypeExpr>>,
    /// i32, u32, u8 = (java::int) <-
    assigned_to: Box<SpannedContainer<TypeExpr>>,
}

impl AbstractMultiAssign {
    pub fn new(
        to_assign: Vec<SpannedContainer<TypeExpr>>,
        assigned_to: Box<SpannedContainer<TypeExpr>>,
    ) -> Self {
        Self {
            to_assign,
            assigned_to,
        }
    }
}
