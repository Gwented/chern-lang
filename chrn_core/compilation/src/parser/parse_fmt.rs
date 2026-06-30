use chrn_utils::intern::Intern;
use lang::fmter::Formattable;

use crate::token::Token;

/// Helper to reduce boiler-plate of formatting a given token
///
/// * type `bool` returns true when an identifier was formatted, false if it was made with a basic
/// token kind.
/// So, if `:` was found, it would return false since there is no identifier.
pub(super) fn fmt_tok(tok: Token, interner: &Intern) -> String {
    let fmtted = match tok {
        Token::Def => "`@def`".to_string(),
        Token::End => "`@end`".to_string(),
        Token::Id(name_id)
        | Token::Str(name_id)
        | Token::Integer(name_id, _)
        | Token::Float(name_id, _) => {
            let ident = interner.search(name_id);
            format!("\"{ident}\"")
        }
        Token::Keyword(kw) => format!("keyword `{}`", kw.to_fmt().to_string()),
        Token::Invalid(name_id) => {
            let invalid_msg = interner.search(name_id);
            let new_msg = format!("invalid token \"{invalid_msg}\"");
            new_msg
        }
        Token::Char(ch) => format!("'{ch}'"),
        Token::BoolLiteral(boolean) => format!("bool literal `{}`", boolean),
        t => format!("`{}`", t.kind()),
    };

    fmtted
}
