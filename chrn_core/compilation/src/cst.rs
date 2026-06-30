use crate::{lexer::token::Token, parser::ast::ast_concepts::Item};

//TODO: For external tooling. The slicing of source and bloating the ast is not worth the pain.
pub enum CSTKind {
    Item(Item),
    Token(Token),
}

// ??
pub fn parse() {}
