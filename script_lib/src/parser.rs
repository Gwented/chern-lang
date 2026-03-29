//TODO: Modifiers with bitwise flags
pub mod ast;
mod context;
mod error;
mod parse_state;
use crate::parser::ast::{
    AbstractAlias, AbstractEnum, AbstractStruct, AbstractTypeDef, AbstractVariant, AstInfo, Call,
    Expr, Generic, Item, SpannedExpr, TypeExpr, Unary, UnaryOp,
};
use crate::parser::context::Context;
use crate::parser::error::Branch;
use crate::parser::parse_state::StateFlag;
use crate::types::symbols::SpannedToken;
use crate::types::token::{Token, TokenKind};
use common::fmter::Formatted;
use common::intern::Intern;
use common::keywords::{self, Keyword};
use common::metadata::ChernMetadata;
use common::symbols::{InnerArgs, NameId, Span, SpannedInnerArgs};

// May be lower
const MAX_ERRORS: u8 = 3;

pub fn parse(metadata: &ChernMetadata, tokens: &Vec<SpannedToken>, interner: &Intern) -> AstInfo {
    let mut ast_info = AstInfo::new();

    let mut state = StateFlag::new();

    let mut ctx = Context::new(&metadata, tokens);

    while ctx.pos < ctx.toks.len() {
        if ctx.err_vec.len() > 10 {
            break;
        }

        let is_priv = match parse_export(&mut ctx, interner) {
            Ok(res) => res,
            Err(_) => continue,
        };

        let tok = ctx.peek_tok();

        match tok {
            Token::Id(id) => match id {
                id if id == Keyword::Bind as u32 => {
                    // May turn into a helper but fine for now
                    if !is_priv {
                        report_export(&mut ctx, Formatted::Bind, Branch::Neutral, interner);
                    }

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
                            "Found a `bind` statement more than once",
                            Branch::Neutral,
                            interner,
                        );

                        continue;
                    } else {
                        state.flip_alias();
                    }

                    _ = parse_alias_stmt(&mut ctx, is_priv, &mut ast_info, interner);
                }
                id if id == Keyword::Const as u32 => {
                    ctx.advance_tok();

                    _ = parse_const(&mut ctx, interner);
                    todo!();
                }
                id if id == Keyword::Var as u32 => {
                    if !is_priv {
                        report_export(&mut ctx, Formatted::Bind, Branch::Neutral, interner);
                    }

                    ctx.advance_tok();

                    //WARN: This was moved which is FINE but A_BASE_EXIT_SET must NOT change or this breaks
                    //Could lead to less detailed error messages so may put back in its place.
                    if state.has_var() {
                        ctx.report_verbose(
                            "Found `var` section more than once",
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
                            && ctx.peek_ahead(1).tok.kind() == TokenKind::SlimArrow
                        {
                            break;
                        }

                        if let Ok(ty) = parse_var_sect(&mut ctx, interner) {
                            ast_info.items.push(Item::Var(ty));
                        }
                    }
                }
                id if id == Keyword::Nest as u32 => {
                    if !is_priv {
                        report_export(&mut ctx, Formatted::Var, Branch::Searching, interner);
                    }

                    ctx.advance_tok();

                    if state.has_nest() {
                        ctx.report_verbose(
                            "Found `nest` section more than once",
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
                            && ctx.peek_ahead(1).tok.kind() == TokenKind::SlimArrow
                        {
                            break;
                        }

                        if let Ok(item) = parse_nest_sect(&mut ctx, interner) {
                            ast_info.items.push(item);
                        }
                    }
                }
                id if id == Keyword::Complex as u32 => {
                    if !is_priv {
                        report_export(&mut ctx, Formatted::Nest, Branch::Searching, interner);
                    }

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
                            && ctx.peek_ahead(1).tok.kind() == TokenKind::SlimArrow
                        {
                            break;
                        }

                        _ = parse_complex_sect(&mut ctx, interner);
                    }
                }
                id if id == Keyword::Override as u32 => {
                    if !is_priv {
                        report_export(&mut ctx, Formatted::Complex, Branch::Searching, interner);
                    }

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
                    // Please lint empty sections please emit 40000 warns for slightly misplaced
                    // spaces

                    while ctx.peek_kind() != TokenKind::EOF {
                        // This would look simpler with keywords
                        if let Token::Id(name_id) = ctx.peek_tok()
                            && keywords::is_sect(name_id)
                            && ctx.peek_ahead(1).tok.kind() == TokenKind::SlimArrow
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
    is_priv: bool,
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

    //WARN: Make sure this works
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

    let alias = AbstractAlias::new(name_id, name_span, params, conds, args, is_priv);
    dbg!(&alias);

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
            if ctx.peek_kind() == TokenKind::Tilde {
                ctx.advance_tok();
            }

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

            ctx.expect_verbose(
                TokenKind::OCurlyBracket,
                &format!("Expected a '{{' to define struct \"{struct_name}\", found "),
                "",
                Branch::Nest,
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

            ctx.expect_verbose(
                TokenKind::OCurlyBracket,
                &format!("Expected a '{{' to define enum \"{enum_name}\", found"),
                "",
                Branch::Nest,
                interner,
            )?;

            let name_id = NameId::new(name);

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

//TODO:
fn parse_complex_sect(ctx: &mut Context, interner: &Intern) -> Result<(), Token> {
    todo!()
}

//TODO:
fn parse_override_sect(ctx: &mut Context, interner: &Intern) -> Result<(), Token> {
    todo!()
}

fn parse_const(ctx: &mut Context, interner: &Intern) -> Result<(), Token> {
    ctx.expect_verbose(
        TokenKind::Id,
        "Expected an identifier after `const`, found ",
        "",
        Branch::Neutral,
        interner,
    )?;

    ctx.expect_verbose(
        TokenKind::Assign,
        "Expected a '=' to declare const value, found ",
        "",
        Branch::Neutral,
        interner,
    )?;

    let expr = parse_expr(ctx, interner)?;
    todo!();
}

fn parse_expr(ctx: &mut Context, interner: &Intern) -> Result<Expr, Token> {
    let next = ctx.peek_ahead(1).tok;

    if next == Token::Plus
        || next == Token::Hyphen
        || next == Token::Asterisk
        || next == Token::Slash
    {
        let expr = parse_term(ctx, interner)?;
    }

    todo!();
}

fn parse_primary(ctx: &mut Context, interner: &Intern) {}

fn parse_term(ctx: &mut Context, interner: &Intern) -> Result<Expr, Token> {
    todo!();
}

// ENFORCE TYPE NAMING FOR GENERICS AT LEAST
fn parse_type(ctx: &mut Context, interner: &Intern) -> Result<TypeExpr, Token> {
    match ctx.peek_tok() {
        Token::Id(id) if ctx.peek_ahead(1).tok.kind() == TokenKind::OAngleBracket => {
            let start = ctx.advance_span().start;

            let name_id = NameId::new(id);

            // Needs to return the end for US
            let (args, end) = parse_generic(ctx, interner)?;
            let generic = Generic::new(name_id, args);

            let span = Span::new(start, end);

            Ok(TypeExpr::Generic(generic, span))
        }
        Token::Tilde => {
            let span = ctx.advance_span();

            let plain_id = ctx.expect_id_verbose(
                TokenKind::Id,
                "Expected an identifier for an escaped type, found ",
                "",
                Branch::VarType,
                interner,
            )?;

            let name_id = NameId::new(plain_id);

            Ok(TypeExpr::Escaped(name_id, span))
        }
        Token::OParen => parse_tuple(ctx, interner),
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

/// Parses assuming that within "List<i32>" the "List" part was skipped
//WARN: USING BASIC SPAN IMPLEMENTATION AND MAY CHANGE
fn parse_generic(ctx: &mut Context, interner: &Intern) -> Result<(Vec<TypeExpr>, usize), Token> {
    ctx.expect_verbose(
        TokenKind::OAngleBracket,
        "Expected a '<' to declare generic, found ",
        "",
        Branch::VarType,
        interner,
    )?;

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

/// This returns a vector
/// Handles everything and does not expect anything to be skipped
//TEST:
fn parse_tuple(ctx: &mut Context, interner: &Intern) -> Result<TypeExpr, Token> {
    let start = ctx.peek_span().start;

    ctx.expect_verbose(
        TokenKind::OParen,
        "Expected a '(' to declare tuple, found ",
        "",
        Branch::VarType,
        interner,
    )?;

    let mut tuple: Vec<TypeExpr> = Vec::new();

    while ctx.peek_kind() == TokenKind::Id || ctx.peek_kind() == TokenKind::Tilde {
        let ty = parse_type(ctx, interner)?;
        tuple.push(ty);

        if ctx.peek_kind() == TokenKind::CParen {
            break;
        }

        ctx.expect_verbose(
            TokenKind::Comma,
            "Expected a ',' or ')' after type, found ",
            "",
            Branch::NestEnum,
            interner,
        )?;
    }

    let end = ctx.peek_span().end;
    // The loop could never run so expecting is needed
    ctx.expect_verbose(
        TokenKind::CParen,
        "Expected a ',' or ')' after type, found ",
        "",
        Branch::NestEnum,
        interner,
    )?;

    let span = Span::new(start, end);

    let tuple = TypeExpr::Tuple(tuple, span);

    Ok(tuple)
}

fn handle_struct_fields(
    ctx: &mut Context,
    struct_name: &str,
    interner: &Intern,
) -> Result<Vec<AbstractTypeDef>, Token> {
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

    ctx.expect_verbose(
        TokenKind::CCurlyBracket,
        &format!("Expected a '}}' to close struct \"{struct_name}\", found "),
        "",
        Branch::NestType,
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

    //FIX: ALSO SUSPICIOUS
    while ctx.peek_kind() == TokenKind::Id {
        let variant = parse_variant(ctx, interner)?;
        variants.push(variant);

        if ctx.peek_kind() == TokenKind::CCurlyBracket {
            break;
        }

        if ctx.peek_kind() == TokenKind::Comma {
            ctx.advance_tok();
        }

        // ctx.expect_verbose(
        //     TokenKind::Comma,
        //     "Expected a ',' or '}}' after variant, found ",
        //     "",
        //     Branch::NestEnum,
        //     interner,
        // )?;
    }

    ctx.expect_verbose(
        TokenKind::CCurlyBracket,
        &format!("Expected a '}}' to close enum \"{enum_name}\", found "),
        "",
        Branch::NestEnum,
        interner,
    )?;

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

    let tuple_opt: Option<TypeExpr> = if ctx.peek_kind() == TokenKind::OParen {
        let tuple = parse_tuple(ctx, interner)?;
        Some(tuple)
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

    let variant = AbstractVariant::new(name_id, name_span, tuple_opt, conds, args);

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

fn parse_cond(ctx: &mut Context, interner: &Intern) -> Result<SpannedExpr, Token> {
    match ctx.peek_tok() {
        Token::Id(id) if ctx.peek_ahead(1).tok.kind() == TokenKind::OParen => {
            let name_id = NameId::new(id);
            let name_span = ctx.peek_span();

            ctx.skip(2);

            let (args, end) = parse_func(ctx, interner)?;

            let func_span = Span::new(name_span.start, end);

            // let callee = Box::new(Expr::Var(name_id, name_span));

            let call = Expr::Call(Call::new(name_id, args));

            let spanned_expr = SpannedExpr::new(call, func_span);
            Ok(spanned_expr)
        }
        Token::Id(id) => {
            let span = ctx.advance_span();

            let name_id = NameId::new(id);

            let spanned_expr = SpannedExpr::new(Expr::Var(name_id), span);
            Ok(spanned_expr)
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

            let spanned_expr = SpannedExpr::new(Expr::Unary(unary), span);
            //WARN:
            Ok(spanned_expr)
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
fn parse_func(ctx: &mut Context, interner: &Intern) -> Result<(Vec<SpannedExpr>, usize), Token> {
    let mut args: Vec<SpannedExpr> = Vec::new();

    while ctx.peek_kind() != TokenKind::CParen {
        let spanned_expr = match ctx.peek_tok() {
            //TEST:
            Token::Id(id) if ctx.peek_ahead(1).tok.kind() == TokenKind::Assign => {
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

                        SpannedExpr::new(Expr::Integer(num), span)
                    }
                    Token::Float(id, notation) => {
                        let span = ctx.advance_span();
                        let num: f64 = interner
                            .search(id as usize)
                            .parse()
                            .expect("Lexer broke (Float).");

                        SpannedExpr::new(Expr::Float(num), span)
                    }
                    Token::Str(id) => {
                        let span = ctx.advance_span();

                        SpannedExpr::new(Expr::Str(NameId::new(id)), span)
                    }
                    Token::Id(id) | Token::Illegal(id) => {
                        let msg = format!(
                            "Cannot have \"{}\" as a default function parameter",
                            interner.search(id as usize)
                        );

                        ctx.report_verbose(&msg, Branch::Cond, interner);

                        return Err(Token::Poison);
                    }
                    Token::Char(ch) => {
                        let msg = format!("Cannot have \"{}\" within function parameters", ch);

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

                let expr = Expr::Default(name_id, Box::new(default));
                SpannedExpr::new(expr, span)
            }
            Token::Id(id) => {
                let span = ctx.advance_span();
                let name_id = NameId::new(id);

                SpannedExpr::new(Expr::Var(name_id), span)
            }
            Token::Str(id) => {
                let span = ctx.advance_span();
                let name_id = NameId::new(id);

                SpannedExpr::new(Expr::Str(name_id), span)
            }
            Token::Integer(id, _) => {
                let span = ctx.advance_span();
                let num: i64 = interner
                    .search(id as usize)
                    .parse()
                    .expect("Lexer broke (Integer)");

                SpannedExpr::new(Expr::Integer(num), span)
            }
            Token::Float(id, _) => {
                let span = ctx.advance_span();
                let num: f64 = interner
                    .search(id as usize)
                    .parse()
                    .expect("Lexer broke (Float).");

                SpannedExpr::new(Expr::Float(num), span)
            }
            Token::Char(ch) => {
                let span = ctx.advance_span();

                SpannedExpr::new(Expr::Char(ch), span)
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

        args.push(spanned_expr);

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

fn parse_export(ctx: &mut Context, interner: &Intern) -> Result<bool, ()> {
    let mut is_priv = true;

    while let Token::Id(id) = ctx.peek_tok()
        && keywords::is_export(id)
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
