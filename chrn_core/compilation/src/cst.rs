use crate::{lexer::token::Token, parser::ast::ast_concepts::Item};
pub mod cst_concepts;
pub mod cst_exprs;

//TODO: For external tooling. The slicing of source and bloating the ast is not worth the pain.
pub enum CSTItem {
    TypeDef(ConcreteTypeDef),
    // Struct(AbstractStruct),
    // Enum(AbstractEnum),
    // Alias(AbstractAlias),
    // Var(AbstractVar),
    // Config(AbstractConfig),
}

pub struct ConcreteTypeDef {
    // Uh....
    pub ident: Token,
    pub colon: Token,
    pub ty: Token,
    pub comma: Option<Token>,
}

// #[derive(Debug)]
// pub struct AbstractTypeDef {
//     pub name_id: InternedId,
//     pub name_span: SourceSpan,
//     pub sp_ty_expr: SpannedContainer<TypeExpr>,
//     pub conds: Vec<SpannedExpr>,
//     pub directives: Vec<AbstractDirective>,
// }
//
// impl AbstractTypeDef {
//     pub fn new(
//         name_id: InternedId,
//         name_span: SourceSpan,
//         sp_ty_expr: SpannedContainer<TypeExpr>,
//         directives: Vec<AbstractDirective>,
//         conds: Vec<SpannedExpr>,
//     ) -> AbstractTypeDef {
//         AbstractTypeDef {
//             name_id,
//             name_span,
//             sp_ty_expr,
//             directives,
//             conds,
//         }
//     }
// }

// ??
pub fn parse() {}
