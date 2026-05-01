pub mod ast;
mod branch;
mod context;
mod parser_state;

use crate::modules::Module;
use crate::parser::ast::{
    AbstractAlias, AbstractEnum, AbstractMemberAccess, AbstractStruct, AbstractTypeDef,
    AbstractVar, AbstractVariant, AstInfo, Expr, Generic, Item, SpannedExpr, SpannedTypeExpr,
    TypeExpr, Unary, UnaryOp,
};
use crate::parser::branch::{Branch, NeutralBranch, SectionBranch};
use crate::parser::context::Context;
use crate::parser::parser_state::ParserState;
use crate::token::{SpannedToken, Token, TokenKind};
use chrn_utils::id_types::InternedId;
use chrn_utils::inner_args::{InnerArgs, SpannedInnerArgs};
use chrn_utils::intern::Intern;
use chrn_utils::keywords::Keyword;
use common::chrn_settings::ChernSettings;
use common::core_error::ScriptError;
use common::fmter::Formatted;
use common::span::Span;

// May be lower
const MAX_ERRORS: u8 = 3;

/// Returns a completed `AstInfo` on `Ok`. Returns an unfinished `AstInfo` and `ScriptError` on
/// `Err`
pub fn parse(
    settings: &ChernSettings,
    module: &Module,
    tokens: &Vec<SpannedToken>,
    interner: &Intern,
) -> Result<AstInfo, (AstInfo, ScriptError)> {
    let mut ast_info = AstInfo::new();

    let mut state = ParserState::new();
    let mut ctx = Context::new(settings, module, tokens);

    // Skipping @def first since it is recognized as it's own token
    if ctx.peek_tok() == Token::Def {
        ctx.advance_tok();
    }

    while !ctx.peek_kind().is_terminator() {
        //TEST:
        // if ctx.err_vec.len() > 10 {
        //     break;
        // }

        // Checks if there is an export which is only a boolean due to there only being private and
        // public
        let is_priv = match parse_export(&mut ctx, interner) {
            Ok(res) => res,
            Err(_) => continue,
        };

        let tok = ctx.peek_tok();

        match tok {
            Token::Keyword(kw) => match kw {
                Keyword::Bind => {
                    if !is_priv {
                        report_export(
                            &mut ctx,
                            Formatted::Bind,
                            Branch::Neutral(NeutralBranch::Searching),
                            interner,
                        );
                    }

                    ctx.advance_tok();

                    if state.has_bind() {
                        ctx.report_verbose(
                            "Found a bind statement more than once",
                            Branch::Neutral(NeutralBranch::Searching),
                            interner,
                        );

                        continue;
                    } else {
                        state.flip_bind();
                    }

                    _ = check_bind(&mut ctx, &interner);
                }
                Keyword::Alias => {
                    ctx.advance_tok();

                    // Maybe remove
                    if !state.has_alias() {
                        state.flip_alias();
                    }

                    if let Ok(alias) = parse_alias_stmt(&mut ctx, is_priv, interner) {
                        ast_info.items.push(Item::Alias(alias));
                    };
                }
                Keyword::Let => {
                    ctx.advance_tok();

                    if let Ok(abs_var) = parse_let(&mut ctx, is_priv, interner) {
                        ast_info.items.push(Item::Var(abs_var));
                    }
                }
                Keyword::Import => {
                    if !is_priv {
                        report_export(
                            &mut ctx,
                            Formatted::Import,
                            Branch::Neutral(NeutralBranch::Searching),
                            interner,
                        );
                    }

                    ctx.advance_tok();

                    _ = check_import(&mut ctx, interner);
                }
                Keyword::Var => {
                    if !is_priv {
                        report_export(
                            &mut ctx,
                            Formatted::SectVar,
                            Branch::Section(SectionBranch::Searching),
                            interner,
                        );
                    }

                    ctx.advance_tok();

                    //WARN: This was moved which is FINE but A_BASE_EXIT_SET must NOT change or this breaks
                    //Could lead to less detailed error messages so may put back in its place.
                    if state.has_var() {
                        ctx.report_verbose(
                            "Found `var` section more than once",
                            Branch::Section(SectionBranch::Searching),
                            interner,
                        );

                        continue;
                    } else {
                        state.flip_var();
                    }

                    _ = ctx.expect_verbose(
                        TokenKind::SlimArrow,
                        "Expected a '->' after section `var`, found ",
                        "",
                        Branch::Section(SectionBranch::Searching),
                        interner,
                    );

                    while !ctx.peek_kind().is_terminator() {
                        if let Token::Keyword(kw) = ctx.peek_tok()
                            && kw.is_sect()
                        {
                            break;
                        }

                        if let Ok(type_def) = parse_var_sect(&mut ctx, interner) {
                            ast_info.items.push(Item::TypeDef(type_def));
                        }
                    }
                }
                Keyword::Nest => {
                    if !is_priv {
                        report_export(
                            &mut ctx,
                            Formatted::SectNest,
                            Branch::Section(SectionBranch::Searching),
                            interner,
                        );
                    }

                    ctx.advance_tok();

                    if state.has_nest() {
                        ctx.report_verbose(
                            "Found `nest` section more than once",
                            Branch::Section(SectionBranch::Searching),
                            interner,
                        );
                        continue;
                    } else {
                        state.flip_nest();
                    }

                    _ = ctx.expect_verbose(
                        TokenKind::SlimArrow,
                        "Expected a '->' after section `nest`, found ",
                        "",
                        Branch::Section(SectionBranch::Searching),
                        interner,
                    );

                    while !ctx.peek_kind().is_terminator() {
                        if let Token::Keyword(kw) = ctx.peek_tok()
                            && kw.is_sect()
                        {
                            break;
                        }

                        let is_priv = match parse_export(&mut ctx, interner) {
                            Ok(p) => p,
                            Err(_) => continue,
                        };

                        if let Ok(item) = parse_nest_sect(&mut ctx, is_priv, interner) {
                            ast_info.items.push(item);
                        }
                    }
                }
                Keyword::Complex => {
                    if !is_priv {
                        report_export(&mut ctx, Formatted::SectNest, Branch::Searching, interner);
                    }

                    todo!("Complex not done");
                    ctx.advance_tok();

                    if state.has_complex() {
                        ctx.report_verbose(
                            "Found `complex` section more than once",
                            Branch::Section(SectionBranch::Searching),
                            interner,
                        );
                        continue;
                    } else {
                        state.flip_complex();
                    }

                    _ = ctx.expect_verbose(
                        TokenKind::SlimArrow,
                        "Expected a '->' after section `complex`, found ",
                        "",
                        Branch::Section(SectionBranch::Searching),
                        interner,
                    );

                    while !ctx.peek_kind().is_terminator() {
                        if let Token::Keyword(kw) = ctx.peek_tok()
                            && kw.is_sect()
                        {
                            break;
                        }

                        _ = parse_complex_sect(&mut ctx, interner);
                    }
                }
                Keyword::Override => {
                    if !is_priv {
                        report_export(
                            &mut ctx,
                            Formatted::SectComplex,
                            Branch::Searching,
                            interner,
                        );
                    }

                    ctx.advance_tok();

                    if state.has_override() {
                        ctx.report_verbose(
                            "Found `override` section more than once",
                            Branch::Section(SectionBranch::Searching),
                            interner,
                        );
                        continue;
                    } else {
                        state.flip_override();
                    }

                    _ = ctx.expect_verbose(
                        TokenKind::SlimArrow,
                        "Expected a '->' after section `override`, found ",
                        "",
                        Branch::Section(SectionBranch::Searching),
                        interner,
                    );
                    // Please lint empty sections please emit 40000 warns for slightly misplaced
                    // spaces

                    while !ctx.peek_kind().is_terminator() {
                        // This would look simpler with keywords
                        if let Token::Keyword(kw) = ctx.peek_tok()
                            && kw.is_sect()
                        {
                            break;
                        }

                        _ = parse_override_sect(&mut ctx, interner);
                    }
                }
                id => {
                    ctx.advance_tok();

                    let name = interner.search(id as usize);
                    let fmsg = format!("keyword `{name}`");

                    if state.is_neutral() {
                        ctx.report_template(
                            "a statement or section",
                            &fmsg,
                            Branch::Neutral(NeutralBranch::Searching),
                            interner,
                        );
                    } else {
                        ctx.report_template(
                            "a section with a '->' after",
                            &fmsg,
                            Branch::Section(SectionBranch::Searching),
                            interner,
                        );
                    }
                }
            },
            Token::Illegal(id) => {
                ctx.advance_tok();

                let err_str = interner.search(id as usize);

                let msg = format!("Found illegal token {err_str}");

                ctx.report_verbose(&msg, Branch::Broken, interner);
            }
            Token::EOF | Token::End => break,
            //TODO: Neutral routing
            t => {
                // Interesting name..
                let allowed_msg = if state.is_neutral() {
                    "a statement or section"
                } else {
                    "a section"
                };

                let fmsg = match t {
                    Token::Def => "`@def`".to_string(),
                    Token::Id(id)
                    | Token::Str(id)
                    | Token::Integer(id, _)
                    | Token::Float(id, _) => {
                        let name = interner.search(id as usize);
                        format!("{} \"{}\"", t.kind(), name)
                    }
                    t => t.kind().to_string(),
                };

                ctx.advance_tok();
                ctx.report_template(allowed_msg, &fmsg, Branch::Searching, interner);
            }
        }
    }

    if !ctx.err_vec.is_empty() {
        return Err((ast_info, ScriptError::Parser(ctx.err_vec)));
    }

    Ok(ast_info)
}

//FIXME: These sets may be misaligned
fn parse_alias_stmt(
    ctx: &mut Context,
    is_priv: bool,
    interner: &Intern,
) -> Result<AbstractAlias, Token> {
    let name_span = ctx.peek_span();

    let plain_id = ctx.expect_id_verbose(
        TokenKind::Id,
        "Expected an identifier after `alias`, found ",
        "",
        Branch::Neutral(NeutralBranch::Alias),
        interner,
    )?;

    let name_id = InternedId::new(plain_id);

    let err_name = interner.search(plain_id as usize);

    ctx.expect_verbose(
        TokenKind::OParen,
        &format!("Expected parameters to define alias \"{err_name}\", found "),
        "",
        Branch::Neutral(NeutralBranch::Alias),
        interner,
    )?;

    let params = parse_func_decl(ctx, interner)?;

    ctx.expect_verbose(
        TokenKind::Assign,
        &format!("Expected '=' to define alias \"{err_name}\", found "),
        "",
        Branch::Neutral(NeutralBranch::Alias),
        interner,
    )?;

    let conds = if ctx.peek_kind() == TokenKind::OBracket {
        handle_conds(ctx, interner)?
    } else {
        Vec::new()
    };

    let args = if ctx.peek_kind() == TokenKind::HashSymbol {
        handle_args(ctx, interner)?
    } else {
        Vec::new()
    };

    // TODO: The case of "alias x() = " should maybe be an error..?
    // Probably not since that would be a useless Go-level error
    // if conds.is_empty() && args.is_empty() {
    //     ctx.report_verbose("", Branch::Neutral(NeutralBranch::Alias), interner);
    // }

    let alias = AbstractAlias::new(name_id, name_span, params, conds, args, is_priv);

    Ok(alias)
}

fn check_bind(ctx: &mut Context, interner: &Intern) -> Result<(), Token> {
    ctx.expect_id_verbose(
        TokenKind::Str,
        "Expected a string literal after `bind`, found ",
        "",
        // Maybe it is still a branch
        Branch::Neutral(NeutralBranch::Bind),
        interner,
    )?;

    Ok(())
}

fn check_import(ctx: &mut Context, interner: &Intern) -> Result<(), Token> {
    ctx.expect_id_verbose(
        TokenKind::Str,
        "Expected a string literal path, found ",
        "",
        Branch::Neutral(NeutralBranch::Import),
        interner,
    )?;

    if let Token::Keyword(kw) = ctx.peek_tok()
        && kw == Keyword::As
    {
        ctx.advance_tok();
        ctx.expect_id_verbose(
            TokenKind::Id,
            "Expected an alias for the given import after keyword `as`, found ",
            "",
            Branch::Neutral(NeutralBranch::Import),
            interner,
        )?;
    }

    Ok(())
}

fn parse_var_sect(ctx: &mut Context, interner: &Intern) -> Result<AbstractTypeDef, Token> {
    let name_span = ctx.peek_span();

    let plain_id = ctx.expect_id_verbose(
        TokenKind::Id,
        "Expected an identifier to define a type, found ",
        "",
        // Not exactly true that var is being used, just that the type definition flow already
        // exists, so...
        Branch::Section(SectionBranch::Var),
        interner,
    )?;

    let name_id = InternedId::new(plain_id);

    let err_name = interner.search(plain_id as usize);

    ctx.expect_verbose(
        TokenKind::Colon,
        &format!("Expected a ':' after \"{err_name}\" to declare a type, found "),
        "",
        Branch::Section(SectionBranch::Var),
        interner,
    )?;

    let ty_res = parse_type(ctx, interner);

    // WARN: DO NOT PROPOGATE
    let conds_res = if ctx.peek_kind() == TokenKind::OBracket {
        handle_conds(ctx, interner)
    } else {
        Ok(Vec::new())
    };

    let args_res = if ctx.peek_kind() == TokenKind::HashSymbol {
        handle_args(ctx, interner)
    } else {
        Ok(Vec::new())
    };

    if ctx.peek_kind() == TokenKind::Comma {
        ctx.advance_tok();
    }

    //WARN: May this is a little too forgiving
    let ty = ty_res?;
    let conds = conds_res?;
    let args = args_res?;

    let abs_typedef = AbstractTypeDef::new(name_id, name_span, ty, args, conds);

    Ok(abs_typedef)
}

fn parse_nest_sect(ctx: &mut Context, is_priv: bool, interner: &Intern) -> Result<Item, Token> {
    // Wait what is this error?
    let kw = ctx.expect_kw_verbose(
        "Expected the keyword `enum` or `struct`, found ",
        "",
        Branch::Section(SectionBranch::Nest),
        interner,
    )?;

    //TODO: Can likely be done simpler but keep for simplicity
    let item = match kw {
        Keyword::Struct => {
            let name_span = ctx.peek_span();

            let plain_id = ctx.expect_id_verbose(
                TokenKind::Id,
                "Expected an identifier for the given struct, found ",
                "",
                Branch::Section(SectionBranch::Nest),
                interner,
            )?;

            let name_id = InternedId::new(plain_id);

            let struct_name = interner.search(plain_id as usize);

            //FIXME: THIS SHOULD WARN OR SOMETHING OF THAT SORT
            //BUG:
            //FIX:

            ctx.expect_verbose(
                TokenKind::OCurlyBracket,
                &format!("Expected a '{{' to define struct `{struct_name}`, found "),
                "",
                Branch::Section(SectionBranch::Nest),
                interner,
            )?;

            let fields = handle_struct_fields(ctx, struct_name, interner)?;

            let conds = if ctx.peek_kind() == TokenKind::OBracket {
                handle_conds(ctx, interner)?
            } else {
                Vec::new()
            };

            let args = if ctx.peek_kind() == TokenKind::HashSymbol {
                handle_args(ctx, interner)?
            } else {
                Vec::new()
            };

            // Unsure if structures or enums will have fields so just stays for now
            let structure = AbstractStruct::new(name_id, name_span, conds, args, fields, is_priv);

            Item::Struct(structure)
        }
        //FIX: Make this normal
        Keyword::Enum => {
            let name_span = ctx.peek_span();

            let plain_id = ctx.expect_id_verbose(
                TokenKind::Id,
                "Expected an identifier for the given enum, found ",
                "",
                Branch::Section(SectionBranch::Nest),
                interner,
            )?;

            let enum_name = interner.search(plain_id as usize);

            ctx.expect_verbose(
                TokenKind::OCurlyBracket,
                &format!("Expected a '{{' block to define enum `{enum_name}`, found"),
                "",
                Branch::Section(SectionBranch::Nest),
                interner,
            )?;

            let name_id = InternedId::new(plain_id);

            let variants = handle_enum_variants(ctx, enum_name, interner)?;

            let glob_conds = if ctx.peek_kind() == TokenKind::OBracket {
                handle_conds(ctx, interner)?
            } else {
                Vec::new()
            };

            let glob_args = if ctx.peek_kind() == TokenKind::HashSymbol {
                handle_args(ctx, interner)?
            } else {
                Vec::new()
            };

            let enumeration =
                AbstractEnum::new(name_id, name_span, variants, glob_conds, glob_args, is_priv);

            Item::Enum(enumeration)
        }
        _ => {
            let name = interner.search(kw as usize);

            ctx.report_verbose(
                &format!("Expected the keyword `enum` or `struct`, found keyword `{name}`"),
                Branch::Section(SectionBranch::Nest),
                interner,
            );

            return Err(Token::Poison);
        }
    };

    Ok(item)
}

//TODO:
fn parse_complex_sect(ctx: &mut Context, interner: &Intern) -> Result<(), Token> {
    todo!()
}

//TODO:
fn parse_override_sect(ctx: &mut Context, interner: &Intern) -> Result<(), Token> {
    todo!()
}

fn parse_let(ctx: &mut Context, is_priv: bool, interner: &Intern) -> Result<AbstractVar, Token> {
    let name_span = ctx.peek_span();

    let plain_id = ctx.expect_id_verbose(
        TokenKind::Id,
        "Expected an identifier after `let`, found ",
        "",
        Branch::Neutral(NeutralBranch::Let),
        interner,
    )?;

    let name_id = InternedId::new(plain_id);

    ctx.expect_verbose(
        TokenKind::Assign,
        "Expected '=' to declare value, found ",
        "",
        Branch::Neutral(NeutralBranch::Let),
        interner,
    )?;

    let spanned_expr = parse_expr(ctx, 0, interner)?;

    let abs_var = AbstractVar::new(name_id, name_span, spanned_expr, is_priv);

    Ok(abs_var)
}

/// Pratt Parser main entry-point
fn parse_expr(ctx: &mut Context, min_bp: u8, interner: &Intern) -> Result<SpannedExpr, Token> {
    let mut lhs = parse_unary(ctx, interner)?;

    loop {
        if let Some((op, bp)) = ctx.peek_tok().precedence() {
            if bp < min_bp {
                break;
            }

            ctx.advance_tok();

            let rhs = parse_expr(ctx, bp + 1, interner)?;

            let span = Span::new(lhs.span.start, rhs.span.end);
            lhs = SpannedExpr::new(
                Expr::BinaryExpr {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                span,
            );
        } else if ctx.peek_tok() == Token::OParen {
            // Handles conditions. lhs should be a name id or field access which is caught later
            let bp = 100;
            if bp < min_bp {
                break;
            }

            ctx.advance_tok();

            let args = parse_call_args(ctx, interner)?;
            let span = Span::new(lhs.span.start, ctx.peek_behind(1).span.end);

            lhs = SpannedExpr::new(Expr::Call(Box::new(lhs), args), span);
        } else if ctx.peek_kind() == TokenKind::Id && ctx.peek_ahead(1).tok == Token::OParen {
            let bp = 100;
            if bp < min_bp {
                break;
            }

            let call_start = ctx.advance_span();
            ctx.advance_tok();

            let args = parse_call_args(ctx, interner)?;
            let span = Span::new(call_start.start, ctx.peek_behind(1).span.end);

            lhs = SpannedExpr::new(Expr::Call(Box::new(lhs), args), span);
        } else if ctx.peek_tok() == Token::Dot && ctx.peek_ahead(1).tok.kind() == TokenKind::Id {
            // Handles cases like field access
            let bp = 100;
            if bp < min_bp {
                break;
            }

            ctx.advance_tok();

            // Redundant
            let field_id = ctx.expect_id_verbose(
                TokenKind::Id,
                "Expected a field identifier after '.', found ",
                "",
                Branch::Expr,
                interner,
            )?;

            let span = Span::new(lhs.span.start, ctx.peek_behind(1).span.end);

            lhs = SpannedExpr::new(
                Expr::MemberAccess(AbstractMemberAccess::new(
                    Box::new(lhs),
                    InternedId::new(field_id),
                )),
                span,
            );
        } else {
            break;
        }
    }

    Ok(lhs)
}

// For single values, likley lhs
fn parse_primary(ctx: &mut Context, interner: &Intern) -> Result<SpannedExpr, Token> {
    match ctx.peek_tok() {
        Token::OParen => {
            ctx.advance_tok();
            let expr = parse_expr(ctx, 0, interner)?;

            ctx.expect_verbose(
                TokenKind::CParen,
                "Expected ')' to close grouped expression, found ",
                "",
                Branch::Expr,
                interner,
            )?;

            Ok(expr)
        }
        Token::Id(id) if ctx.peek_ahead(1).tok == Token::Assign => {
            let name_id = InternedId::new(id);
            let name_span = ctx.advance_span();

            ctx.advance_tok();

            let expr = parse_unary(ctx, interner)?;

            let span = Span::new(name_span.start, ctx.peek_behind(1).span.end);

            let default = Expr::Default(name_id, Box::new(expr));

            Ok(SpannedExpr::new(default, span))
        }
        Token::BoolLiteral(boolean) => {
            let span = ctx.advance_span();
            Ok(SpannedExpr::new(Expr::Bool(boolean), span))
        }
        Token::Id(id) => {
            let span = ctx.advance_span();

            let name_id = InternedId::new(id);

            Ok(SpannedExpr::new(Expr::Var(name_id), span))
        }
        Token::Integer(id, notation) => {
            let span = ctx.advance_span();

            Ok(SpannedExpr::new(Expr::Integer(id, notation), span))
        }
        Token::Float(id, notation) => {
            let span = ctx.advance_span();

            Ok(SpannedExpr::new(Expr::Float(id, notation), span))
        }
        Token::Str(id) => {
            let span = ctx.advance_span();
            let name_id = InternedId::new(id);

            Ok(SpannedExpr::new(Expr::Str(name_id), span))
        }
        Token::Char(ch) => {
            let span = ctx.advance_span();

            Ok(SpannedExpr::new(Expr::Char(ch), span))
        }
        Token::EOF => {
            ctx.advance_tok();

            ctx.report_verbose(
                "Expected an expression, found <eof>",
                Branch::Type,
                interner,
            );

            Err(Token::EOF)
        }
        t => {
            ctx.advance_tok();

            let msg = match t {
                Token::Illegal(id) => format!(
                    "Expected a valid expression, found illegal \"{}\"",
                    interner.search(id as usize)
                ),
                _ => format!("Expected a valid expression, found \"{}\"", t.kind()),
            };

            ctx.report_verbose(&msg, Branch::Expr, interner);
            return Err(Token::Poison);
        }
    }
}

fn parse_call_args(ctx: &mut Context, interner: &Intern) -> Result<Vec<SpannedExpr>, Token> {
    let mut args: Vec<SpannedExpr> = Vec::new();

    if ctx.peek_kind() == TokenKind::CParen {
        ctx.advance_tok();
        return Ok(args);
    }

    loop {
        let arg = parse_expr(ctx, 0, interner)?;
        args.push(arg);

        if ctx.peek_kind() == TokenKind::CParen {
            ctx.advance_tok();
            break;
        }

        ctx.expect_verbose(
            TokenKind::Comma,
            "Expected ',' to separate arguments or ')' to close, found ",
            "",
            Branch::FuncArgs,
            interner,
        )?;
    }

    Ok(args)
}

fn parse_unary(ctx: &mut Context, interner: &Intern) -> Result<SpannedExpr, Token> {
    match ctx.peek_tok() {
        // May 'let op = if' evenually to combine similar unaries
        Token::Hyphen => {
            let start = ctx.advance_span().start;
            let expr = parse_unary(ctx, interner)?;

            let span = Span::new(start, expr.span.end);
            let unary = Unary::new(UnaryOp::Negate, Box::new(expr));

            Ok(SpannedExpr::new(Expr::Unary(unary), span))
        }
        Token::ExclamationPoint => {
            let start = ctx.advance_span().start;

            let expr = parse_unary(ctx, interner)?;
            let span = Span::new(start, expr.span.end);

            let unary = Unary::new(UnaryOp::Not, Box::new(expr));

            Ok(SpannedExpr::new(Expr::Unary(unary), span))
        }
        _ => parse_primary(ctx, interner),
    }
}

// ENFORCE TYPE NAMING FOR GENERICS AT LEAST
/// Recursive function for parsing all type expressions
fn parse_type(ctx: &mut Context, interner: &Intern) -> Result<SpannedTypeExpr, Token> {
    match ctx.peek_tok() {
        Token::Id(id) if ctx.peek_ahead(1).tok.kind() == TokenKind::OAngleBracket => {
            let start = ctx.advance_span().start;

            let name_id = InternedId::new(id);

            let args = parse_generic(ctx, interner)?;
            let generic = Generic::new(name_id, args);

            let end = ctx.peek_behind(1).span.end;
            let span = Span::new(start, end);

            let ty_expr = TypeExpr::Generic(generic);
            Ok(SpannedTypeExpr::new(ty_expr, span))
        }
        Token::Id(id) if ctx.peek_ahead(1).tok.kind() == TokenKind::Dot => {
            let start = ctx.peek_span().start;
            let ty_path = parse_type_path(ctx, interner)?;
            let end = ctx.peek_behind(1).span.end;

            let span = Span::new(start, end);

            Ok(SpannedTypeExpr::new(TypeExpr::Path(ty_path), span))
        }
        // Token::OParen => parse_tuple(ctx, interner),
        Token::Id(id) => {
            let span = ctx.advance_span();

            //FIX: How is this going to be escaped.
            let name_id = InternedId::new(id);
            let ty_expr = TypeExpr::Var(name_id);

            Ok(SpannedTypeExpr::new(ty_expr, span))
        }
        Token::QuestionMark => {
            let span = ctx.advance_span();

            Ok(SpannedTypeExpr::new(TypeExpr::Any, span))
        }
        Token::Str(id) | Token::Integer(id, _) => {
            let name = interner.search(id as usize);
            let kind = ctx.peek_kind();

            ctx.advance_tok();

            let fmt_tok = format!("{} \"{name}\"", kind);
            ctx.report_template("a type", &fmt_tok, Branch::Type, interner);

            Err(Token::Str(id))
        }
        Token::EOF => {
            ctx.advance_tok();

            ctx.report_verbose("Expected a type, found <eof>", Branch::Type, interner);
            Err(Token::EOF)
        }
        t => {
            ctx.advance_tok();

            let fmt_tok = format!("'{}'", t.kind());

            ctx.report_template("a type", &fmt_tok, Branch::Type, interner);
            //WARN:
            Err(Token::Poison)
        }
    }
}

/// Assumes this function was called after a token of identifier type was found with a dot after
/// it.
fn parse_type_path(ctx: &mut Context, interner: &Intern) -> Result<Vec<SpannedTypeExpr>, Token> {
    let mut ty_path: Vec<SpannedTypeExpr> = Vec::new();

    // Technically only allows 2 type paths total since, there is no scenario where it would be
    // possible otherwise, but this will still allow any amount because it ensures any future
    // development decision to change this will be easier
    loop {
        if ctx.peek_ahead(1).tok == Token::Dot {
            let span = ctx.peek_span();
            let name_id = ctx.expect_id_verbose(
                TokenKind::Id,
                "Expected an identifier after dot reference, found ",
                "",
                Branch::Type,
                interner,
            )?;

            ctx.advance_tok();

            let spanned_ty_expr =
                SpannedTypeExpr::new(TypeExpr::Var(InternedId::new(name_id)), span);
            ty_path.push(spanned_ty_expr);
        }

        if ctx.peek_ahead(1).tok != Token::Dot {
            break;
        }
    }

    //TODO: More intuitive errors or help
    let span = ctx.peek_span();
    let final_id = ctx.expect_id_verbose(
        TokenKind::Id,
        "Expected a complete dot reference chain, found ",
        "",
        Branch::Type,
        interner,
    )?;

    let ty_expr = TypeExpr::Var(InternedId::new(final_id));

    let final_expr = SpannedTypeExpr::new(ty_expr, span);
    ty_path.push(final_expr);

    Ok(ty_path)
}

/// Parses assuming that within "List<i32>" the "List" part was skipped, which would leave <i32>
/// to be handled
//WARN: USING BASIC SPAN IMPLEMENTATION AND MAY CHANGE
fn parse_generic(ctx: &mut Context, interner: &Intern) -> Result<Vec<SpannedTypeExpr>, Token> {
    ctx.expect_verbose(
        TokenKind::OAngleBracket,
        "Expected a '<' to declare generic, found ",
        "",
        Branch::Type,
        interner,
    )?;

    let mut args: Vec<SpannedTypeExpr> = Vec::new();

    let arg = parse_type(ctx, interner)?;
    args.push(arg);

    while ctx.peek_kind() == TokenKind::Comma {
        ctx.advance_tok();

        let other_arg = parse_type(ctx, interner)?;
        args.push(other_arg);
    }

    ctx.expect_verbose(
        TokenKind::CAngleBracket,
        "Expected a '>' to close generic parameters, found ",
        "",
        Branch::Type,
        interner,
    )?;

    Ok(args)
}

// //TEST:
// fn parse_tuple(ctx: &mut Context, interner: &Intern) -> Result<TypeExpr, Token> {
//     let start = ctx.peek_span().start;
//
//     ctx.expect_verbose(
//         TokenKind::OParen,
//         "Expected a '(' to declare tuple, found ",
//         "",
//         Branch::VarType,
//         interner,
//     )?;
//
//     let mut tuple: Vec<TypeExpr> = Vec::new();
//
//     while ctx.peek_kind() == TokenKind::Id {
//         let ty = parse_type(ctx, interner)?;
//         tuple.push(ty);
//
//         if ctx.peek_kind() == TokenKind::CParen {
//             break;
//         }
//
//         ctx.expect_verbose(
//             TokenKind::Comma,
//             "Expected a ',' or ')' after type, found ",
//             "",
//             Branch::NestEnum,
//             interner,
//         )?;
//     }
//
//     let end = ctx.peek_span().end;
//     // The loop could never run so expecting is needed
//     ctx.expect_verbose(
//         TokenKind::CParen,
//         "Expected a ',' or ')' after type, found ",
//         "",
//         Branch::NestEnum,
//         interner,
//     )?;
//
//     let span = Span::new(start, end);
//
//     let tuple = TypeExpr::Tuple(tuple, span);
//
//     Ok(tuple)
// }

fn handle_struct_fields(
    ctx: &mut Context,
    struct_name: &str,
    interner: &Intern,
) -> Result<Vec<AbstractTypeDef>, Token> {
    let mut fields: Vec<AbstractTypeDef> = Vec::new();

    //WARN: Suspicious loop
    while ctx.peek_kind() == TokenKind::Id {
        let ty = parse_var_sect(ctx, interner)?;
        fields.push(ty);

        // A little too suspicious
        if ctx.peek_kind() == TokenKind::Comma {
            ctx.advance_tok();
        }
    }

    ctx.expect_verbose(
        TokenKind::CCurlyBracket,
        &format!("Expected a field or '}}' to close struct `{struct_name}`, found "),
        "",
        Branch::Section(SectionBranch::NestType),
        interner,
    )?;

    Ok(fields)
}

/// Assumes the leading '{' was skipped
fn handle_enum_variants(
    ctx: &mut Context,
    enum_name: &str,
    interner: &Intern,
) -> Result<Vec<AbstractVariant>, Token> {
    let mut variants: Vec<AbstractVariant> = Vec::new();

    //NOTE: ALSO SUSPICIOUS
    while ctx.peek_kind() == TokenKind::Id {
        let variant = parse_variant(ctx, interner)?;
        variants.push(variant);

        if ctx.peek_kind() == TokenKind::CCurlyBracket {
            break;
        }

        if ctx.peek_kind() == TokenKind::Comma {
            ctx.advance_tok();
        }
    }

    ctx.expect_verbose(
        TokenKind::CCurlyBracket,
        &format!("Expected a variant or '}}' to close enum `{enum_name}`, found "),
        "",
        Branch::Section(SectionBranch::NestEnum),
        interner,
    )?;

    Ok(variants)
}

// This COULD re-use parse_var_sect but not sure yet
fn parse_variant(ctx: &mut Context, interner: &Intern) -> Result<AbstractVariant, Token> {
    let name_span = ctx.peek_span();

    let plain_id = ctx.expect_id_verbose(
        TokenKind::Id,
        "Expected an identifier for a variant, found ",
        "",
        Branch::Section(SectionBranch::NestType),
        interner,
    )?;

    let name_id = InternedId::new(plain_id);

    // TODO: Maybe this shouldn't look like a tuple by default since it's misleading
    let ty_opt: Option<SpannedTypeExpr> = if ctx.peek_kind() == TokenKind::Colon {
        ctx.advance_tok();
        let ty = parse_type(ctx, interner)?;
        Some(ty)
    } else {
        None
    };

    let conds_res = if ctx.peek_kind() == TokenKind::OBracket {
        handle_conds(ctx, interner)
    } else {
        Ok(Vec::new())
    };

    let args_res = if ctx.peek_kind() == TokenKind::HashSymbol {
        handle_args(ctx, interner)
    } else {
        Ok(Vec::new())
    };

    // Might expect..
    if ctx.peek_kind() == TokenKind::Comma {
        ctx.advance_tok();
    }

    let conds = conds_res?;
    let args = args_res?;

    let variant = AbstractVariant::new(name_id, name_span, ty_opt, conds, args);

    Ok(variant)
}

// Egregious naming scheme
fn handle_args(ctx: &mut Context, interner: &Intern) -> Result<Vec<SpannedInnerArgs>, Token> {
    let mut args: Vec<SpannedInnerArgs> = Vec::new();

    let mut err_count = 0;

    while ctx.peek_kind() == TokenKind::HashSymbol {
        ctx.advance_tok();

        let arg = parse_arg(ctx, interner);

        if let Ok(arg) = arg {
            args.push(arg);
        } else {
            if err_count > MAX_ERRORS {
                break;
            }

            err_count += 1;
        }
    }

    Ok(args)
}

fn parse_arg(ctx: &mut Context, interner: &Intern) -> Result<SpannedInnerArgs, Token> {
    let name_span = ctx.peek_span();

    let id = ctx.expect_id_verbose(
        TokenKind::Id,
        "",
        " is not a valid argument.",
        Branch::TypeArgs,
        interner,
    )?;

    // FIX: Change from try_from to Some
    let arg = InnerArgs::try_from(interner.search(id as usize)).or_else(|invalid_arg| {
        let msg = format!("The argument \"#{invalid_arg}\" does not exist");
        ctx.report_verbose(&msg, Branch::TypeArgs, interner);

        return Err(Token::Poison);
    })?;

    Ok(SpannedInnerArgs::new(arg, name_span))
}

fn parse_func_decl(ctx: &mut Context, interner: &Intern) -> Result<Vec<SpannedTypeExpr>, Token> {
    let mut args: Vec<SpannedTypeExpr> = Vec::new();

    while ctx.peek_kind() != TokenKind::CParen {
        let expr = match ctx.peek_tok() {
            Token::Id(id) => {
                let span = ctx.advance_span();
                let ty_expr = TypeExpr::Var(InternedId::new(id));

                SpannedTypeExpr::new(ty_expr, span)
            }
            Token::EOF => return Err(Token::Poison),
            t => {
                ctx.advance_tok();

                let msg = match t {
                    Token::Illegal(id) => format!(
                        "Cannot have \"{}\" within alias definition",
                        interner.search(id as usize)
                    ),
                    _ => format!("Cannot have '{}' within alias definition", t.kind()),
                };

                ctx.report_verbose(&msg, Branch::Cond, interner);
                return Err(Token::Poison);
            }
        };

        args.push(expr);

        if ctx.peek_kind() == TokenKind::CParen {
            break;
        }

        _ = ctx.expect_verbose(
            TokenKind::Comma,
            "Expected a ',' to separate arguments or ')' to close, found ",
            "",
            Branch::Cond,
            interner,
        )?;
    }

    ctx.expect_verbose(
        TokenKind::CParen,
        "Expected ')' to close declaration, found ",
        "",
        Branch::FuncArgs,
        interner,
    )?;

    Ok(args)
}

fn handle_conds(ctx: &mut Context, interner: &Intern) -> Result<Vec<SpannedExpr>, Token> {
    let mut conds: Vec<SpannedExpr> = Vec::new();
    // This count cannot end the definition since it would prevent arguments from being viewed
    let mut err_count = 0;

    ctx.expect_verbose(
        TokenKind::OBracket,
        "Expected a '[' to define conditions, found ",
        "",
        Branch::Cond,
        interner,
    )?;

    if ctx.peek_kind() == TokenKind::CBracket {
        ctx.advance_tok();
        return Ok(conds);
    }

    loop {
        let new_cond = parse_expr(ctx, 0, interner);

        if let Ok(cond) = new_cond {
            conds.push(cond);
        } else {
            if err_count > MAX_ERRORS {
                break;
            }

            err_count += 1;
        }

        // Should be able to send help since ctx would know a comma was used after a cond
        if ctx.peek_kind() != TokenKind::Comma {
            break;
        }

        ctx.advance_tok();
    }

    if err_count == 0 {
        _ = ctx.expect_verbose(
            TokenKind::CBracket,
            "Expected ']' at end of condition, found ",
            "",
            // Does this set align properly?
            Branch::Cond,
            interner,
        );
    };

    Ok(conds)
}

//NOTE: Could make the first pass only resolve the basics so that module resolution for local
//imports are handled without giving the module passer too much syntax knowledge, but fine for now.
/// Only does syntax checking for import since modules are resolved on first pass.
fn parse_export(ctx: &mut Context, interner: &Intern) -> Result<bool, ()> {
    let mut is_priv = true;

    while let Token::Keyword(kw) = ctx.peek_tok()
        && kw == Keyword::Export
    {
        if !is_priv {
            ctx.advance_tok();

            ctx.report_verbose(
                "Cannot use `export` more than once at a time",
                Branch::Searching,
                interner,
            );

            return Err(());
        } else {
            ctx.advance_tok();
            is_priv = false;
        }
    }

    Ok(is_priv)
}

/// Helper for solely reporting export
fn report_export(ctx: &mut Context, fmtted: Formatted, branch: Branch, interner: &Intern) {
    ctx.report_verbose(
        &format!("Cannot use `export` on `{}`", fmtted),
        branch,
        interner,
    );
}
