//FIXME: All top level structures need to be able to take arguments
// Cond should be a proper function for all
pub mod ast;
mod context;
// Unpub this
pub mod error;
pub mod parse_state;

use crate::parser::ast::{
    AbstractAlias, AbstractEnum, AbstractGeneric, AbstractStruct, AbstractTypeDef, AbstractVariant,
    AstInfo, Call, Expr, Item, TypeExpr, Unary, UnaryOp,
};
use crate::parser::context::Context;
use crate::parser::error::Branch;
use crate::parser::parse_state::StateFlag;
use crate::types::symbols::SpannedToken;
use crate::types::token::{Token, TokenKind};
use common::intern::Intern;
use common::keywords::{self, Keyword};
use common::metadata::FileMetadata;
use common::symbols::{InnerArgs, NameId, Span, SpannedInnerArgs};

// May be lower
const MAX_ERRORS: u8 = 3;

pub fn parse(metadata: &FileMetadata, tokens: &Vec<SpannedToken>, interner: &Intern) -> AstInfo {
    let mut ast_info = AstInfo::new();

    let mut state = StateFlag::new();

    let mut ctx = Context::new(&metadata, tokens);

    while ctx.pos < ctx.tokens.len() {
        if ctx.err_vec.len() > 10 {
            break;
        }

        let tok = ctx.peek_tok();

        match tok {
            Token::Id(id) => match id {
                id if id == Keyword::Bind as u32 => {
                    ctx.advance_tok();

                    if state.has_bind() {
                        ctx.report_verbose(
                            "Found a bind statement more than once",
                            Branch::Neutral,
                            interner,
                        );

                        continue;
                    } else {
                        state.flip_bind();
                    }

                    _ = parse_bind_stmt(&mut ctx, &mut ast_info, interner);
                }
                id if id == Keyword::Alias as u32 => {
                    ctx.advance_tok();

                    if state.has_alias() {
                        ctx.report_verbose(
                            "Found a bind statement more than once",
                            Branch::Neutral,
                            interner,
                        );

                        continue;
                    } else {
                        state.flip_alias();
                    }

                    _ = parse_alias_stmt(&mut ctx, &mut ast_info, interner);
                }
                id if id == Keyword::Var as u32 => {
                    ctx.advance_tok();

                    //WARN: This was moved which is FINE but A_BASE_EXIT_SET must NOT change or this breaks
                    //Could lead to less detailed error messages so may put back in its place.
                    if state.has_var() {
                        ctx.report_verbose(
                            "Found \"var\" section more than once",
                            Branch::Searching,
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
                        Branch::Searching,
                        interner,
                    );

                    while ctx.peek_kind() != TokenKind::EOF {
                        if let Token::Id(plain_id) = ctx.peek_tok()
                            && keywords::is_sect(plain_id)
                                // Oh my
                            && ctx.peek_ahead(1).token.kind() == TokenKind::SlimArrow
                        {
                            break;
                        }

                        if let Ok(ty) = parse_var_sect(&mut ctx, interner) {
                            ast_info.items.push(Item::Var(ty));
                        }
                    }
                }
                id if id == Keyword::Nest as u32 => {
                    ctx.advance_tok();

                    if state.has_nest() {
                        ctx.report_verbose(
                            "Found \"nest\" section more than once",
                            Branch::Searching,
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
                        //TODO: Better help
                        Branch::Searching,
                        interner,
                    );

                    while ctx.peek_kind() != TokenKind::EOF {
                        if let Token::Id(name_id) = ctx.peek_tok()
                            && keywords::is_sect(name_id)
                            && ctx.peek_ahead(1).token.kind() == TokenKind::SlimArrow
                        {
                            break;
                        }

                        if let Ok(item) = parse_nest_sect(&mut ctx, interner) {
                            ast_info.items.push(item);
                        }
                    }
                }
                id if id == Keyword::Complex as u32 => {
                    todo!("Complex not done");
                    ctx.advance_tok();

                    if state.has_complex() {
                        ctx.report_verbose(
                            "Found \"complex\" section more than once",
                            Branch::Searching,
                            interner,
                        );
                        continue;
                        if state.has_complex() {
                            ctx.report_verbose(
                                "Found \"complex\" section more than once",
                                Branch::Complex,
                                interner,
                            );
                            continue;
                        } else {
                            state.flip_complex();
                        }
                    } else {
                        state.flip_complex();
                    }

                    _ = ctx.expect_verbose(
                        TokenKind::SlimArrow,
                        "Expected a '->' after section `complex`, found ",
                        "",
                        Branch::Searching,
                        interner,
                    );

                    while ctx.peek_kind() != TokenKind::EOF {
                        if let Token::Id(name_id) = ctx.peek_tok()
                            && keywords::is_sect(name_id)
                            && ctx.peek_ahead(1).token.kind() == TokenKind::SlimArrow
                        {
                            break;
                        }

                        _ = parse_complex_sect(&mut ctx, interner);
                    }
                }
                id if id == Keyword::Override as u32 => {
                    todo!("Override not done");
                    ctx.advance_tok();

                    if state.has_override() {
                        ctx.report_verbose(
                            "Found \"override\" section more than once",
                            Branch::Searching,
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
                        Branch::Searching,
                        interner,
                    );

                    while ctx.peek_kind() != TokenKind::EOF {
                        if let Token::Id(name_id) = ctx.peek_tok()
                            && keywords::is_sect(name_id)
                            && ctx.peek_ahead(1).token.kind() == TokenKind::SlimArrow
                        {
                            break;
                        }

                        _ = parse_override_sect(&mut ctx, interner);
                    }
                }
                id => {
                    ctx.advance_tok();

                    let name = interner.search(id as usize);
                    let fmsg = format!("identifier \"{name}\"");

                    ctx.report_template(
                        "a section with a '->' after",
                        &fmsg,
                        Branch::Searching,
                        interner,
                    );
                }
            },
            Token::Illegal(id) => {
                ctx.advance_tok();

                let err_str = interner.search(id as usize);

                let msg = format!("Found illegal token {err_str}");

                ctx.report_verbose(&msg, Branch::Broken, interner);
            }
            Token::EOF => break,
            t => match t {
                Token::Id(id) | Token::Str(id) | Token::Integer(id, _) => {
                    ctx.advance_tok();

                    let name = interner.search(id as usize);
                    let fmsg = format!("{} \"{}\"", t.kind(), name);

                    ctx.report_template("a section", &fmsg, Branch::Searching, interner);
                }
                _ => {
                    ctx.advance_tok();
                    let fmsg = format!("'{}'", t.kind());
                    ctx.report_template("a section", &fmsg, Branch::Searching, interner);
                }
            },
        }
    }

    if !ctx.err_vec.is_empty() {
        ctx.emit_errors();
        std::process::exit(1);
    }

    dbg!(&ast_info);
    ast_info
}

//FIXME: These sets may be misaligned
fn parse_alias_stmt(
    ctx: &mut Context,
    ast_info: &mut AstInfo,
    interner: &Intern,
) -> Result<(), Token> {
    let name_span = ctx.peek_span();

    let plain_id = ctx.expect_id_verbose(
        TokenKind::Id,
        "Expected an identifier after \"alias\", found ",
        "",
        Branch::Neutral,
        interner,
    )?;

    let name_id = NameId::new(plain_id);

    //NOTE: ignore this
    // let err_name = || -> &str {interner.search(name_id as usize)};

    let err_name = interner.search(plain_id as usize);

    ctx.expect_verbose(
        TokenKind::OParen,
        // WHAT IF THIS WAS LAZY?
        &format!("Expected '(' to define alias \"{err_name}\", found "),
        "",
        // May not need corresponding set
        Branch::Neutral,
        interner,
    )?;

    // Keeping this in case
    let start = ctx.peek_span().start;

    let (params, end) = parse_func_decl(ctx, interner)?;

    ctx.expect_verbose(
        TokenKind::Assign,
        &format!("Expected '=' to define alias \"{err_name}\", found "),
        "",
        Branch::Neutral,
        interner,
    )?;

    ctx.expect_verbose(
        TokenKind::OBracket,
        "Expected a '[' for conditions, found ",
        "",
        Branch::Neutral,
        interner,
    )?;

    let conds = handle_conds(ctx, interner)?;

    let alias = AbstractAlias::new(name_id, name_span, params, conds);

    ast_info.items.push(Item::Alias(alias));

    Ok(())
}

fn parse_bind_stmt(
    ctx: &mut Context,
    ast_info: &mut AstInfo,
    interner: &Intern,
) -> Result<(), Token> {
    let name_id = ctx.expect_id_verbose(
        TokenKind::Literal,
        "Expected a string literal after `bind`, found ",
        "",
        // Maybe it is still a branch
        Branch::Bind,
        interner,
    )?;

    let name_id = NameId::new(name_id);

    ast_info.set_bind(name_id);

    Ok(())
}

fn parse_var_sect(ctx: &mut Context, interner: &Intern) -> Result<AbstractTypeDef, Token> {
    let name_span = ctx.peek_span();

    let plain_id = ctx.expect_id_verbose(
        TokenKind::Id,
        "Expected an identifier to declare a type, found ",
        "",
        Branch::Var,
        interner,
    )?;

    let name_id = NameId::new(plain_id);

    let err_name = interner.search(plain_id as usize);

    dbg!(ctx.peek_tok());

    ctx.expect_verbose(
        TokenKind::Colon,
        &format!("Expected a ':' after identifier \"{err_name}\" to declare a type, found "),
        "",
        Branch::Var,
        interner,
    )?;

    let type_res = parse_type(ctx, interner);

    // WARN: DO NOT PROPOGATE
    let conds_res = if ctx.peek_kind() == TokenKind::OBracket {
        ctx.advance_tok();
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
    let ty = type_res?;
    let conds = conds_res?;
    let args = args_res?;

    let abs_typedef = AbstractTypeDef::new(name_id, name_span, ty, args, conds);

    Ok(abs_typedef)
}

fn parse_nest_sect(ctx: &mut Context, interner: &Intern) -> Result<Item, Token> {
    // Wait what is this error?
    let id = ctx.expect_id_verbose(
        TokenKind::Id,
        "Expected the keyword \"enum\" or \"struct\", found ",
        "",
        Branch::Nest,
        interner,
    )?;

    //TODO: Can likely be done simpler but keep for simplicity
    let item = match id {
        id if id == Keyword::Struct as u32 => {
            let name_span = ctx.peek_span();

            let name = ctx.expect_id_verbose(
                TokenKind::Id,
                "Expected an identifier for the given structure. found ",
                "",
                Branch::Nest,
                interner,
            )?;

            let struct_name = interner.search(name as usize);

            let name_id = NameId::new(name);

            let fields = handle_struct_fields(ctx, struct_name, interner)?;

            let conds = if ctx.peek_kind() == TokenKind::OBracket {
                ctx.advance_tok();
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
            let structure = AbstractStruct::new(name_id, name_span, conds, args, fields);

            Item::Struct(structure)
        }
        id if id == Keyword::Enum as u32 => {
            let name_span = ctx.peek_span();

            let name = ctx.expect_id_verbose(
                TokenKind::Id,
                "Expected an identifier for the given enum. found ",
                "",
                Branch::Nest,
                interner,
            )?;

            let enum_name = interner.search(name as usize);

            let name_id = NameId::new(name);

            let variants = handle_enum_variants(ctx, enum_name, interner)?;

            let glob_conds = if ctx.peek_kind() == TokenKind::OBracket {
                ctx.advance_tok();
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
                AbstractEnum::new(name_id, name_span, variants, glob_conds, glob_args);

            Item::Enum(enumeration)
        }
        _ => {
            let name = interner.search(id as usize);

            ctx.report_verbose(
                &format!(
                    "Expected the keyword \"enum\" or \"struct\", found identifier \"{name}\""
                ),
                Branch::NestType,
                interner,
            );

            return Err(Token::Poison);
        }
    };

    Ok(item)
}

fn parse_complex_sect(ctx: &mut Context, interner: &Intern) -> Result<(), Token> {
    todo!()
}

fn parse_override_sect(ctx: &mut Context, interner: &Intern) -> Result<(), Token> {
    todo!()
}

// ENFORCE TYPE NAMING FOR GENERICS AT LEAST
fn parse_type(ctx: &mut Context, interner: &Intern) -> Result<TypeExpr, Token> {
    match ctx.peek_tok() {
        Token::Id(id) if ctx.peek_ahead(1).token.kind() == TokenKind::OAngleBracket => {
            let start = ctx.peek_span().start;

            ctx.skip(2);

            let name_id = NameId::new(id);

            // Needs to return the end for US
            let (args, end) = parse_generic(ctx, interner)?;
            let generic = AbstractGeneric::new(name_id, args);

            let span = Span::new(start, end);

            Ok(TypeExpr::Generic(generic, span))
        }
        Token::Id(id) => {
            let span = ctx.advance_span();

            let name_id = NameId::new(id);

            Ok(TypeExpr::Var(name_id, span))
        }
        Token::QuestionMark => {
            let span = ctx.advance_span();

            Ok(TypeExpr::Any(span))
        }
        Token::Str(id) | Token::Integer(id, _) => {
            let name = interner.search(id as usize);

            let kind = ctx.peek_kind();

            ctx.advance_tok();

            let fmt_tok = format!("{} \"{name}\"", kind);
            ctx.report_template("a type", &fmt_tok, Branch::VarType, interner);

            Err(Token::Str(id))
        }
        Token::EOF => {
            //FIX: Points to EOF since it is technically the error.
            ctx.advance_tok();

            ctx.report_verbose("Expected type, found <eof>", Branch::VarType, interner);
            Err(Token::EOF)
        }
        Token::Poison => {
            panic!("Touched <poison>");
        }
        t => {
            dbg!(ctx.peek_tok());
            ctx.advance_tok();

            let fmt_tok = format!("'{}'", t.kind());

            ctx.report_template("a type", &fmt_tok, Branch::VarType, interner);
            //WARN:
            Err(Token::Poison)
        }
    }
}

//WARN: USING BASIC SPAN IMPLEMENTATION AND MAY CHANGE
fn parse_generic(ctx: &mut Context, interner: &Intern) -> Result<(Vec<TypeExpr>, usize), Token> {
    let mut args: Vec<TypeExpr> = Vec::new();

    let arg_one = parse_type(ctx, interner)?;
    args.push(arg_one);

    if ctx.peek_kind() == TokenKind::Comma {
        ctx.advance_tok();
        let arg_two = parse_type(ctx, interner)?;
        args.push(arg_two);
    }

    let end = ctx.peek_span().end;

    ctx.expect_verbose(
        TokenKind::CAngleBracket,
        "Expected a '>' to close generic parameters, found ",
        "",
        Branch::VarType,
        interner,
    )?;

    Ok((args, end))
}

fn handle_struct_fields(
    ctx: &mut Context,
    struct_name: &str,
    interner: &Intern,
) -> Result<Vec<AbstractTypeDef>, Token> {
    _ = ctx.expect_verbose(
        TokenKind::OCurlyBracket,
        &format!("Expected a '{{' before defining struct \"{struct_name}\", found "),
        "",
        Branch::NestType,
        interner,
    );

    let mut fields: Vec<AbstractTypeDef> = Vec::new();

    //FIXME: Suspicious loop
    while ctx.peek_kind() == TokenKind::Id {
        let ty = parse_var_sect(ctx, interner)?;
        fields.push(ty);

        // A little too suspicious
        if ctx.peek_kind() == TokenKind::Comma {
            ctx.advance_tok();
        }
    }

    _ = ctx.expect_verbose(
        TokenKind::CCurlyBracket,
        &format!("Expected a '}}' to close struct \"{struct_name}\", found "),
        "",
        Branch::NestType,
        interner,
    );

    Ok(fields)
}

fn handle_enum_variants(
    ctx: &mut Context,
    enum_name: &str,
    interner: &Intern,
) -> Result<Vec<AbstractVariant>, Token> {
    _ = ctx.expect_verbose(
        TokenKind::OCurlyBracket,
        &format!("Expected a '{{' before defining the enum \"{enum_name}\", found "),
        "",
        Branch::NestType,
        interner,
    );

    let mut variants: Vec<AbstractVariant> = Vec::new();

    //FIX: ALSO SUSPICIOUS
    while ctx.peek_kind() == TokenKind::Id {
        let variant = parse_variant(ctx, interner)?;
        variants.push(variant);

        if ctx.peek_kind() == TokenKind::Comma {
            ctx.advance_tok();
        }
    }

    _ = ctx.expect_verbose(
        TokenKind::CCurlyBracket,
        &format!("Expected a '}}' to close enum \"{enum_name}\", found "),
        "",
        Branch::NestEnum,
        interner,
    );

    Ok(variants)
}

fn parse_variant(ctx: &mut Context, interner: &Intern) -> Result<AbstractVariant, Token> {
    let name_span = ctx.peek_span();

    let name = ctx.expect_id_verbose(
        TokenKind::Id,
        "Expected an identifier for a variant, found ",
        "",
        Branch::NestType,
        interner,
    )?;

    let name_id = NameId::new(name);

    let err_name = interner.search(name as usize);

    //TODO: Allow comma separated types
    let type_opt = if ctx.peek_kind() == TokenKind::OParen {
        ctx.advance_tok();

        let type_span = ctx.peek_span();

        let type_id = ctx.expect_id_verbose(
            TokenKind::Id,
            &format!("Expected a type within variant \"{err_name}\", found "),
            "",
            Branch::NestEnum,
            interner,
        )?;

        let type_name = NameId::new(type_id);

        ctx.expect_verbose(
            TokenKind::CParen,
            &format!("Expected a ')' to close variant \"{err_name}\", found "),
            "",
            Branch::NestEnum,
            interner,
        )?;

        Some(TypeExpr::Var(type_name, type_span))
    } else {
        None
    };

    let conds_res = if ctx.peek_kind() == TokenKind::OBracket {
        ctx.advance_tok();
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

    let conds = conds_res?;
    let args = args_res?;

    let variant = AbstractVariant::new(name_id, name_span, type_opt, conds, args);

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
        Branch::VarTypeArgs,
        interner,
    )?;

    // FIX: Change from try_from to Some
    let arg = InnerArgs::try_from(interner.search(id as usize)).or_else(|invalid_id| {
        let msg = format!("The argument \"#{invalid_id}\" does not exist");
        ctx.report_verbose(&msg, Branch::VarTypeArgs, interner);

        return Err(Token::Poison);
    })?;

    Ok(SpannedInnerArgs::new(arg, name_span))
}

fn parse_cond(ctx: &mut Context, interner: &Intern) -> Result<Expr, Token> {
    match ctx.peek_tok() {
        Token::Id(id) if ctx.peek_ahead(1).token.kind() == TokenKind::OParen => {
            let name_id = NameId::new(id);
            let name_span = ctx.peek_span();

            ctx.skip(2);

            let (args, end) = parse_func(ctx, interner)?;

            let func_span = Span::new(name_span.start, end);

            // let callee = Box::new(Expr::Var(name_id, name_span));
            //
            Ok(Expr::Call(Call::new(name_id, args), func_span))
        }
        Token::Id(id) => {
            let span = ctx.advance_span();

            let name_id = NameId::new(id);

            Ok(Expr::Var(name_id, span))
        }
        Token::Str(id) | Token::Integer(id, _) | Token::Float(id, _) | Token::Illegal(id) => {
            let err_tok = ctx.advance_tok();

            let name = interner.search(id as usize);

            let fmt_tok = format!("{} \"{name}\"", err_tok.kind());
            ctx.report_template(
                "a condition after declared type",
                &fmt_tok,
                Branch::Cond,
                interner,
            );

            //WARN:
            Err(Token::Poison)
        }
        Token::Char(ch) => {
            let err_tok = ctx.advance_tok();

            let fmt_tok = format!("{} \"{ch}\"", err_tok.kind());
            ctx.report_template(
                "a condition after declared type",
                &fmt_tok,
                Branch::Cond,
                interner,
            );

            //WARN:
            Err(Token::Poison)
        }
        Token::ExclamationPoint => {
            //FIXME: SPAN IS INCOMPLETE
            let span = ctx.advance_span();

            let wrapped = parse_cond(ctx, interner)?;

            let unary = Unary::new(UnaryOp::Not, Box::new(wrapped));

            //WARN:
            Ok(Expr::Unary(unary, span))
        }
        t => {
            ctx.advance_tok();

            let fmt_tok = format!("'{}'", t.kind());
            ctx.report_template("a valid condition", &fmt_tok, Branch::Cond, interner);

            Err(t)
        }
    }
}

//TODO: Should this be terminal?
// Should this innately check for open parenthesis, or should that be handled at the call site?
fn parse_func(ctx: &mut Context, interner: &Intern) -> Result<(Vec<Expr>, usize), Token> {
    let mut args: Vec<Expr> = Vec::new();

    while ctx.peek_kind() != TokenKind::CParen {
        let expr = match ctx.peek_tok() {
            Token::Id(id) if ctx.peek_ahead(1).token.kind() == TokenKind::Assign => {
                let span = ctx.peek_span();
                let name_id = NameId::new(id);
                // Skipping var id and equals
                ctx.skip(2);

                let default = match ctx.peek_tok() {
                    Token::Integer(id, notation) => {
                        let span = ctx.advance_span();
                        let num: i64 = interner
                            .search(id as usize)
                            .parse()
                            .expect("Lexer broke (Integer)");

                        Expr::Integer(num, span)
                    }
                    Token::Float(id, notation) => {
                        let span = ctx.advance_span();
                        let num: f64 = interner
                            .search(id as usize)
                            .parse()
                            .expect("Lexer broke (Float).");

                        Expr::Float(num, span)
                    }
                    Token::Str(id) => {
                        let span = ctx.advance_span();
                        Expr::Str(NameId::new(id), span)
                    }
                    Token::Id(id) | Token::Illegal(id) | Token::Illegal(id) => {
                        let msg = format!(
                            "Cannot have \"{}\" within function parameters",
                            interner.search(id as usize)
                        );

                        ctx.report_verbose(&msg, Branch::Cond, interner);

                        return Err(Token::Poison);
                    }
                    Token::Char(ch) => {
                        let msg = format!(
                            "Cannot have \"{}\" within function parameters",
                            interner.search(id as usize)
                        );

                        ctx.report_verbose(&msg, Branch::Cond, interner);

                        return Err(Token::Poison);
                    }
                    Token::EOF => return Err(Token::Poison),
                    t => {
                        ctx.advance_tok();

                        let msg = match t {
                            Token::Illegal(id) => format!(
                                "Cannot have \"{}\" within function parameters",
                                interner.search(id as usize)
                            ),
                            _ => format!("Cannot have '{}' within function parameters", t.kind()),
                        };

                        ctx.report_verbose(&msg, Branch::Cond, interner);
                        return Err(Token::Poison);
                    }
                };

                Expr::Default((name_id, span), Box::new(default))
            }
            Token::Id(id) => {
                let span = ctx.advance_span();
                Expr::Var(NameId::new(id), span)
            }
            Token::Str(id) => {
                let span = ctx.advance_span();
                Expr::Str(NameId::new(id), span)
            }
            Token::Integer(id, _) => {
                let span = ctx.advance_span();
                let num: i64 = interner
                    .search(id as usize)
                    .parse()
                    .expect("Lexer broke (Integer)");

                Expr::Integer(num, span)
            }
            Token::Float(id, _) => {
                let span = ctx.advance_span();
                let num: f64 = interner
                    .search(id as usize)
                    .parse()
                    .expect("Lexer broke (Float).");

                Expr::Float(num, span)
            }
            Token::Char(ch) => {
                let span = ctx.advance_span();

                Expr::Char(ch, span)
            }
            Token::EOF => return Err(Token::Poison),
            t => {
                ctx.advance_tok();

                let msg = match t {
                    Token::Illegal(id) => format!(
                        "Cannot have \"{}\" within function parameters",
                        interner.search(id as usize)
                    ),
                    _ => format!("Cannot have '{}' within function parameters", t.kind()),
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

    let end = ctx.advance_span().end;

    Ok((args, end))
}

fn parse_func_decl(ctx: &mut Context, interner: &Intern) -> Result<(Vec<TypeExpr>, usize), Token> {
    let mut args: Vec<TypeExpr> = Vec::new();

    while ctx.peek_kind() != TokenKind::CParen {
        let expr = match ctx.peek_tok() {
            Token::Id(id) => {
                let span = ctx.advance_span();
                TypeExpr::Var(NameId::new(id), span)
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

    let end = ctx.advance_span().end;

    Ok((args, end))
}

fn handle_conds(ctx: &mut Context, interner: &Intern) -> Result<Vec<Expr>, Token> {
    let mut conds: Vec<Expr> = Vec::new();
    // This count cannot end the definition since it would prevent arguments from being viewed
    let mut err_count = 0;

    //FIX: Make this stop on first error. Maybe.
    loop {
        let new_cond = parse_cond(ctx, interner);

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
        //BUG: Cont: 'e' is valid, but EOF is hit when ']' is expected. Why is EOF not being
        //higlighted? We are AT EOF. RIGHT here.
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
