pub mod ast;
pub mod context;
pub mod error;
pub mod symbols;

use std::env::var;

use crate::parser::ast::{
    AbstractBind, AbstractEnum, AbstractFunc, AbstractGeneric, AbstractStruct, AbstractType, Call,
    Expr, Field, Generic, Item, TypeExpr, Unary, UnaryOp, Variant,
};
use crate::parser::error::Branch;
use crate::{
    parser::context::Context,
    token::{SpannedToken, Token, TokenKind},
};
use common::intern::Intern;
use common::primitives::PrimitiveKeywords;
use common::symbols::{InnerArgs, NameId};

// May be lower
const MAX_ERRORS: u8 = 3;

pub fn parse(original_text: &[u8], tokens: &Vec<SpannedToken>, interner: &Intern) -> Vec<Item> {
    let mut ast: Vec<Item> = Vec::new();

    let mut ctx = Context::new(original_text, tokens);

    while ctx.pos < ctx.tokens.len() {
        if ctx.err_vec.len() > 10 {
            break;
        }

        let tok = ctx.peek_tok();

        match tok {
            Token::Id(id) => match id {
                id if id == PrimitiveKeywords::Bind as u32 => {
                    ctx.advance_tok();

                    _ = ctx.expect_verbose(
                        TokenKind::SlimArrow,
                        "Expected a '->' after section `bind`, found ",
                        "",
                        None,
                        Branch::Searching,
                        interner,
                    );

                    parse_bind_sect(&mut ctx, &mut ast, interner).ok();
                }
                id if id == PrimitiveKeywords::Var as u32 => {
                    ctx.advance_tok();

                    _ = ctx.expect_verbose(
                        TokenKind::SlimArrow,
                        "Expected a '->' after section `var`, found ",
                        "",
                        None,
                        Branch::Searching,
                        interner,
                    );

                    //TODO: Fix loop to not stop until another section is seen
                    while ctx.peek_kind() != TokenKind::EOF {
                        if let Token::Id(plain_id) = ctx.peek_tok()
                            && interner.is_section(plain_id)
                                // Oh my
                            && ctx.peek_ahead(1).token.kind() != TokenKind::Colon
                        {
                            break;
                        }

                        if let Ok(ty) = parse_var_sect(&mut ctx, interner) {
                            ast.push(Item::Var(ty));
                        }
                    }
                }
                id if id == PrimitiveKeywords::Nest as u32 => {
                    ctx.advance_tok();

                    _ = ctx.expect_verbose(
                        TokenKind::SlimArrow,
                        "Expected a '->' after section `nest`, found ",
                        "",
                        //TODO: Better help
                        None,
                        Branch::Searching,
                        interner,
                    );

                    while ctx.peek_kind() != TokenKind::EOF {
                        // May hallucinate an error where a colon is present making it seem as
                        // though you cannot name errors section names
                        if let Token::Id(name_id) = ctx.peek_tok()
                            && interner.is_section(name_id)
                            && ctx.peek_ahead(1).token.kind() == TokenKind::SlimArrow
                        {
                            break;
                        }
                        dbg!(ctx.peek_tok());

                        _ = parse_nest_sect(&mut ctx, interner);
                    }
                }
                id if id == PrimitiveKeywords::Complex as u32 => {
                    todo!();
                    ctx.advance_tok();

                    _ = ctx.expect_verbose(
                        TokenKind::SlimArrow,
                        "Expected a '->' after section `complex_rules`, found ",
                        "",
                        None,
                        Branch::Searching,
                        interner,
                    );

                    while ctx.peek_kind() != TokenKind::EOF {
                        if let Token::Id(name_id) = ctx.peek_tok()
                            && interner.is_section(name_id)
                            && ctx.peek_ahead(1).token.kind() == TokenKind::SlimArrow
                        {
                            break;
                        }

                        _ = parse_nest_sect(&mut ctx, interner);
                    }
                }
                id => {
                    //FIX: CHECK FOR SIMILARITY
                    ctx.advance_tok();

                    let name = interner.search(id as usize);
                    let fmsg = format!("identifier \"{name}\"");

                    ctx.report_template("a section with a '->' after", &fmsg, Branch::Searching);
                }
            },
            Token::Illegal(id) => {
                ctx.advance_tok();

                let err_str = interner.search(id as usize);

                let msg = format!("Found illegal token {err_str}");

                ctx.report_verbose(&msg, None, Branch::Broken);
            }
            Token::EOF => break,
            t => match t {
                Token::Id(id) | Token::Literal(id) | Token::Number(id) => {
                    ctx.advance_tok();

                    let name = interner.search(id as usize);
                    let fmsg = format!("{} \"{}\"", t.kind(), name);

                    ctx.report_template("a section or type definition", &fmsg, Branch::Searching);
                }
                _ => {
                    ctx.advance_tok();
                    let fmsg = format!("'{}'", t.kind());
                    ctx.report_template("a section or type definition", &fmsg, Branch::Searching);
                }
            },
        }
    }

    if !ctx.err_vec.is_empty() {
        dbg!(ast);

        //FIX: ANSI
        // Should I even be using this macro?
        // Also this is odd fix it.
        eprintln!("From path: {}", file!());
        eprint!("\x1b[31mError\x1b[0m: ");

        for err in ctx.err_vec.iter() {
            eprintln!("{}\n", err.msg);
        }

        eprintln!("Reported {} error(s)\n", ctx.err_vec.len());
        std::process::exit(1);
    }

    dbg!(&ast);
    ast
}

fn parse_bind_sect(ctx: &mut Context, ast: &mut Vec<Item>, interner: &Intern) -> Result<(), Token> {
    let name_id = ctx.expect_id_verbose(
        TokenKind::Literal,
        "Expected a string literal within section `bind`, found ",
        "",
        Branch::Bind,
        interner,
    )?;

    let name_id = NameId::new(name_id);

    let bind = Item::Bind(AbstractBind::new(name_id));

    ast.push(bind);

    Ok(())
}

fn parse_var_sect(ctx: &mut Context, interner: &Intern) -> Result<AbstractType, Token> {
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
        None,
        Branch::VarType,
        interner,
    )?;

    let type_res = parse_type(ctx, interner);

    let mut conds: Vec<Expr> = Vec::new();
    // This count cannot end the definition since it would prevent arguments from being viewed
    let mut err_count = 0;

    //FIX: Make this stop on first error. Maybe.
    if ctx.peek_kind() == TokenKind::OBracket {
        ctx.advance_tok();

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
            _ = ctx.expect_verbose(
                TokenKind::CBracket,
                "Expected ']' at end of condition, found ",
                "",
                None,
                // Does this set align properly?
                Branch::VarCond,
                interner,
            );
        }
    }
    dbg!(&conds);

    let mut args: Vec<InnerArgs> = Vec::new();

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

    if ctx.peek_kind() == TokenKind::Comma {
        ctx.advance_tok();
    }

    let ty = type_res?;

    let ty = AbstractType::new(name_id, ty, args, conds);

    Ok(ty)
}

// ENFORCE TYPE NAMING FOR GENERICS AT LEAST
fn parse_type(ctx: &mut Context, interner: &Intern) -> Result<TypeExpr, Token> {
    match ctx.peek_tok() {
        // TODO: Maybe make this iterative
        Token::Id(id) if ctx.peek_ahead(1).token.kind() == TokenKind::OAngleBracket => {
            ctx.skip(2);

            let name_id = NameId::new(id);

            let args = parse_generic(ctx, interner)?;
            let generic = Generic::new(name_id, args);

            Ok(TypeExpr::Generic(generic))
        }
        Token::Id(id) => {
            ctx.advance_tok();

            let name_id = NameId::new(id);

            Ok(TypeExpr::Var(name_id))
        }
        Token::QuestionMark => {
            ctx.advance_tok();

            Ok(TypeExpr::Any)
        }
        Token::Literal(id) | Token::Number(id) => {
            let name = interner.search(id as usize);
            let kind = ctx.peek_kind();

            ctx.advance_tok();

            let fmt_tok = format!("{} \"{name}\"", kind);
            ctx.report_template("a type", &fmt_tok, Branch::VarType);

            Err(Token::Literal(id))
        }
        Token::EOF => {
            //FIX: Points to EOF since it is technically the error.
            ctx.advance_tok();

            ctx.report_verbose("Expected type, found '<eof>'", None, Branch::VarType);
            Err(Token::EOF)
        }
        Token::Poison => {
            panic!("Touched <poison>");
        }
        //TODO:
        t => {
            dbg!(ctx.peek_tok());
            ctx.advance_tok();

            let fmt_tok = format!("'{}'", t.kind());

            ctx.report_template("a type", &fmt_tok, Branch::VarType);
            //WARN:
            Err(Token::Poison)
        }
    }
}

fn parse_generic(ctx: &mut Context, interner: &Intern) -> Result<Vec<TypeExpr>, Token> {
    let mut args: Vec<TypeExpr> = Vec::new();

    // farg <-
    let ty = parse_type(ctx, interner)?;
    args.push(ty);

    if ctx.peek_kind() == TokenKind::Comma {
        ctx.advance_tok();
        let ty = parse_type(ctx, interner)?;
        // sarg <-
        args.push(ty);
    }

    ctx.expect_verbose(
        TokenKind::CAngleBracket,
        "Expected a '>' to close generic parameters, found ",
        "",
        None,
        Branch::VarType,
        interner,
    )?;

    Ok(args)
}

fn parse_arg(ctx: &mut Context, interner: &Intern) -> Result<InnerArgs, Token> {
    let id = ctx.expect_id_verbose(
        TokenKind::Id,
        "",
        " is not a valid argument identifier. |e.g. #warn|",
        Branch::VarTypeArgs,
        interner,
    )?;

    InnerArgs::try_from(interner.search(id as usize)).or_else(|invalid_id| {
        let msg = format!("The argument \"#{invalid_id}\" does not exist");
        ctx.report_verbose(&msg, None, Branch::VarTypeArgs);

        return Err(Token::Poison);
    })
}

// TODO: Maybe not need ast passed everywhere
fn parse_cond(ctx: &mut Context, interner: &Intern) -> Result<Expr, Token> {
    match ctx.peek_tok() {
        Token::Id(id) => {
            match PrimitiveKeywords::from_id(id) {
                Some(prim) => match prim {
                    //TODO: Use or for this..
                    //Can likely funnel this all into a singular function that just returns Expr
                    //which could be var or call making this less odd
                    PrimitiveKeywords::IsEmpty => {
                        ctx.advance_tok();

                        let name_id = NameId::new(id);

                        Ok(Expr::Var(name_id))
                    }
                    PrimitiveKeywords::IsWhitespace => {
                        ctx.advance_tok();

                        let name_id = NameId::new(id);

                        Ok(Expr::Var(name_id))
                    }
                    _ => {
                        ctx.advance_tok();

                        let name_id = NameId::new(id);

                        let func_name = interner.search(id as usize);

                        let args = handle_func_args(ctx, func_name, interner)?;

                        let callee = Box::new(Expr::Var(name_id));

                        //WARN: Could be wrong
                        Ok(Expr::Call(Call::new(callee, args)))
                    }
                },
                //FIX:
                None => {
                    ctx.advance_tok();

                    let name_id = NameId::new(id);

                    let func_name = interner.search(id as usize);

                    let args = handle_func_args(ctx, func_name, interner)?;

                    let callee = Box::new(Expr::Var(name_id));

                    //WARN: Could be wrong
                    Ok(Expr::Call(Call::new(callee, args)))
                }
            }
        }
        Token::Literal(id) | Token::Number(id) => {
            let name = interner.search(id as usize);

            let fmt_tok = format!("{} \"{name}\"", TokenKind::Literal);
            ctx.report_template("a condition after declared type", &fmt_tok, Branch::VarCond);

            //WARN:
            Err(Token::Poison)
        }
        Token::ExclamationPoint => {
            //TODO: Probably should just use booleans this is a bit much
            ctx.advance_tok();

            if ctx.peek_kind() == TokenKind::ExclamationPoint {
                ctx.report_template(
                    "a valid condition",
                    "another '!'. `Not` can only be used once in a single statement.",
                    Branch::VarCond,
                );
                //WARN:
                return Err(Token::Poison);
            }

            let wrapped = parse_cond(ctx, interner)?;
            dbg!(&wrapped);

            let unary = Unary::new(UnaryOp::Not, Box::new(wrapped));

            Ok(Expr::Unary(unary))
        }
        t => {
            ctx.advance_tok();

            let fmt_tok = format!("'{}'", t.kind());
            ctx.report_template("a valid condition", &fmt_tok, Branch::VarCond);

            Err(t)
        }
    }
}

//TODO: Make this a general function that just returns expr for keywords and funcs to be AMONG
//Or not I don't know.
fn handle_func_args(
    ctx: &mut Context,
    func_name: &str,
    interner: &Intern,
) -> Result<Vec<Expr>, Token> {
    // Should this be terminal?
    _ = ctx.expect_verbose(
        TokenKind::OParen,
        // Bit convoluted
        &format!("Expected '(' to declare parameters for the function \"{func_name}\", found "),
        "",
        None,
        Branch::VarFuncArgs,
        interner,
    );

    let mut args: Vec<Expr> = Vec::new();

    while ctx.peek_kind() != TokenKind::CParen {
        match ctx.peek_tok() {
            Token::Id(id) => {
                ctx.advance_tok();

                let name_id = NameId::new(id);

                args.push(Expr::Var(name_id));
            }
            Token::Literal(id) => {
                ctx.advance_tok();

                // Should be called something more close to value maybe?
                let name_id = NameId::new(id);

                args.push(Expr::Literal(name_id));
            }
            Token::Number(id) => {
                ctx.advance_tok();

                //WARN: Maybe change this later to remain a string but ok for now
                let num: usize = interner.search(id as usize).parse().expect("Lexer broke");

                args.push(Expr::Number(num));
            }
            Token::CParen => break,
            Token::EOF => return Err(Token::Poison),
            err_tok => {
                ctx.advance_tok();

                let msg = format!(
                    "Cannot have '{}' within function parameters",
                    err_tok.kind()
                );

                ctx.report_verbose(&msg, None, Branch::VarCond);
                return Err(Token::Poison);
            }
        }

        if ctx.peek_kind() == TokenKind::CParen {
            break;
        }

        _ = ctx.expect_verbose(
            TokenKind::Comma,
            "Expected a ',' to separate arguments or ')' to close, found ",
            "",
            None,
            Branch::VarFuncArgs,
            interner,
        )?;
    }

    ctx.advance_tok();

    Ok(args)
}

fn parse_nest_sect(ctx: &mut Context, interner: &Intern) -> Result<Item, Token> {
    let id = ctx.expect_id_verbose(
        TokenKind::Id,
        "Expected a keyword, found ",
        "",
        Branch::Nest,
        interner,
    )?;

    //TODO: Can likely be done simpler but keep for simplicity
    let item = match id {
        id if id == PrimitiveKeywords::Struct as u32 => {
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

            // Unsure if structures or enums will have fields so just stays for now
            let structure = AbstractStruct::new(name_id, Vec::new(), Vec::new(), fields);

            Item::Struct(structure)
        }
        id if id == PrimitiveKeywords::Enum as u32 => {
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

            ctx.advance_tok();

            let enumeration = AbstractEnum::new(name_id, Vec::new(), Vec::new(), variants);

            Item::Enum(enumeration)
        }
        _ => {
            ctx.report_verbose("Expected ", None, Branch::NestType);
            return Err(Token::Poison);
        }
    };

    Ok(item)
}

fn handle_struct_fields(
    ctx: &mut Context,
    struct_name: &str,
    interner: &Intern,
) -> Result<Vec<AbstractType>, Token> {
    _ = ctx.expect_verbose(
        TokenKind::OCurlyBracket,
        &format!("Expected a '{{' before defining struct \"{struct_name}\", found "),
        "",
        None,
        Branch::NestType,
        interner,
    );

    let mut fields: Vec<AbstractType> = Vec::new();

    // Suspicious loop
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
        None,
        Branch::NestType,
        interner,
    );

    Ok(fields)
}

fn handle_enum_variants(
    ctx: &mut Context,
    enum_name: &str,
    interner: &Intern,
) -> Result<Vec<Variant>, Token> {
    _ = ctx.expect_verbose(
        TokenKind::OCurlyBracket,
        &format!("Expected a '{{' before defining the enum \"{enum_name}\", found "),
        "",
        None,
        Branch::NestType,
        interner,
    );

    let mut variants: Vec<Variant> = Vec::new();

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
        None,
        Branch::NestType,
        interner,
    );

    Ok(variants)
}

//FIXME: Has to be some way to handle this better without copy and pasting from parse_var
fn parse_variant(ctx: &mut Context, interner: &Intern) -> Result<Variant, Token> {
    let name = ctx.expect_id_verbose(
        TokenKind::Id,
        "Expected an identifier for a variant, found ",
        "",
        Branch::NestType,
        interner,
    )?;

    let name_id = NameId::new(name);

    let err_name = interner.search(name as usize);

    //WARN: Names are very messy here
    let ty_opt = if ctx.peek_kind() == TokenKind::OParen {
        ctx.advance_tok();

        let type_id = ctx.expect_id_verbose(
            TokenKind::Id,
            &format!("Expected a type within variant \"{err_name}\", found "),
            "",
            Branch::NestType,
            interner,
        )?;

        let type_name = NameId::new(type_id);

        ctx.expect_verbose(
            TokenKind::CParen,
            &format!("Expected a ')' to close variant \"{err_name}\", found "),
            "",
            None,
            Branch::NestType,
            interner,
        )?;

        Some(TypeExpr::Var(type_name))
    } else {
        None
    };

    if ctx.peek_kind() == TokenKind::OBracket {
        ctx.advance_tok();

        let mut conds: Vec<Expr> = Vec::new();

        let mut err_count = 0;

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
                None,
                // Does this set align properly?
                Branch::VarCond,
                interner,
            );
        }
    }

    let mut args: Vec<InnerArgs> = Vec::new();

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

    if ctx.peek_kind() == TokenKind::Comma {
        ctx.advance_tok();
    }

    let variant = Variant::new(name_id, ty_opt, args, Vec::new());

    Ok(variant)
}

// fn parse_complex_section(
//     ctx: &mut Context,
//     interner: &Intern,
//     sym_table: &mut SymbolTable,
// ) -> Result<(), Token> {
//     todo!()
// }
