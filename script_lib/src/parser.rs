pub mod ast;
pub mod context;
pub mod error;
pub mod symbols;

use crate::parser::ast::{
    AbstractBind, AbstractFunc, AbstractGeneric, AbstractType, Call, Expr, Generic, Item, TypeExpr,
    Unary, UnaryOp,
};
use crate::parser::error::Branch;
use crate::parser::symbols::FuncArgs;
use crate::{
    parser::context::Context,
    token::{SpannedToken, Token, TokenKind},
};
use common::intern::Intern;
use common::primitives::PrimitiveKeywords;
use common::symbols::{Cond, InnerArgs, NameId, SymbolId};

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

                    parse_bind_section(&mut ctx, &mut ast, interner).ok();
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

                        if let Ok(ty) = parse_var_section(&mut ctx, &mut ast, interner) {
                            // panic!();
                            // let sym_id = NameId::new(type_def.sym_id.id);
                            //
                            // let type_id = ast.store_typedef(type_def);
                            //
                            // ast.store_symbol(sym_id, Symbol::Def(type_id));
                        }
                    }
                }
                id if id == PrimitiveKeywords::Nest as u32 => {
                    todo!();
                    ctx.advance_tok();

                    ctx.expect_verbose(
                        TokenKind::SlimArrow,
                        "Expected a '->' after section `nest`, found ",
                        "",
                        //TODO: Better help
                        None,
                        Branch::Searching,
                        interner,
                    )
                    .ok();

                    while ctx.peek_kind() != TokenKind::EOF {
                        // May hallucinate an error where a colon is present making it seem as
                        // though you cannot name errors section names
                        if let Token::Id(name_id) = ctx.peek_tok()
                            && interner.is_section(name_id)
                            && ctx.peek_ahead(1).token.kind() == TokenKind::SlimArrow
                        {
                            break;
                        }

                        // parse_nest_section(&mut ctx, interner, &mut ast).ok();
                    }
                }
                id if id == PrimitiveKeywords::ComplexRules as u32 => {
                    todo!();
                    ctx.advance_tok();

                    ctx.expect_verbose(
                        TokenKind::SlimArrow,
                        "Expected a '->' after section `complex_rules`, found ",
                        "",
                        None,
                        Branch::Searching,
                        interner,
                    )
                    .ok();

                    while ctx.peek_kind() != TokenKind::EOF {
                        if let Token::Id(name_id) = ctx.peek_tok()
                            && interner.is_section(name_id)
                            && ctx.peek_ahead(1).token.kind() == TokenKind::SlimArrow
                        {
                            break;
                        }

                        // parse_nest_section(&mut ctx, interner, &mut ast).ok();
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

fn parse_bind_section(
    ctx: &mut Context,
    ast: &mut Vec<Item>,
    interner: &Intern,
) -> Result<(), Token> {
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

// Cutoff

fn parse_var_section(
    ctx: &mut Context,
    ast: &mut Vec<Item>,
    interner: &Intern,
) -> Result<Item, Token> {
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

    //TODO: starts ***-----------------------------------------------------------------------***

    //BUG: Not returning Err and persists in being Ok(())
    let type_res = parse_type(ctx, ast, interner);

    let mut conds: Vec<Expr> = Vec::new();
    // This count cannot end the definition since it would prevent arguments from being viewed
    let mut err_count = 0;

    //FIX: Make this stop on first error
    if ctx.peek_kind() == TokenKind::OBracket {
        ctx.advance_tok();

        loop {
            let new_cond = parse_cond(ctx, ast, interner);

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
            ctx.expect_verbose(
                TokenKind::CBracket,
                "Expected ']' at end of condition, found ",
                "",
                None,
                // Does this set align properly?
                Branch::VarCond,
                interner,
            )
            .ok();
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

    // //TEST: Everything is poison checked by the err_vec check at the end so this SHOULD be fine
    // let type_def = AbstractType::new(name_id, raw_type, args, conds);

    let ty = type_res?;

    let item = Item::Var(AbstractType::new(name_id, ty, args, conds));

    Ok(item)
}

// ENFORCE TYPE NAMING FOR GENERICS AT LEAST
fn parse_type(
    ctx: &mut Context,
    ast: &mut Vec<Item>,
    interner: &Intern,
) -> Result<TypeExpr, Token> {
    match ctx.peek_tok() {
        // TODO: Maybe make this iterative
        Token::Id(id) if ctx.peek_ahead(1).token.kind() == TokenKind::OAngleBracket => {
            ctx.skip(2);

            let name_id = NameId::new(id);

            let args = parse_generic(ctx, ast, interner)?;
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

fn parse_generic(
    ctx: &mut Context,
    ast: &mut Vec<Item>,
    interner: &Intern,
) -> Result<Vec<TypeExpr>, Token> {
    let mut args: Vec<TypeExpr> = Vec::new();

    //TODO: Enforce only two types allowed at most Meaning Map<str,str,str> would just quit

    // farg <-
    let ty = parse_type(ctx, ast, interner)?;
    args.push(ty);

    if ctx.peek_kind() == TokenKind::Comma {
        ctx.advance_tok();
        let ty = parse_type(ctx, ast, interner)?;
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

        return Err(Token::Illegal(id));
    })
}

// TODO: Maybe not need ast passed everywhere
fn parse_cond(ctx: &mut Context, ast: &mut Vec<Item>, interner: &Intern) -> Result<Expr, Token> {
    match ctx.peek_tok() {
        Token::Id(id) => {
            match PrimitiveKeywords::from_id(id) {
                //FIX: Will maybe separate keyword types so that this is easier to handle without
                //code repetition.
                Some(prim) => match prim {
                    //TODO: Use or for this..
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

            let wrapped = parse_cond(ctx, ast, interner)?;
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

//TODO: Cleaner way to report with function name noted. If not that's ok.
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
//
// fn parse_nest_section(
//     ctx: &mut Context,
//     interner: &Intern,
//     sym_table: &mut SymbolTable,
// ) -> Result<(), Token> {
//     ctx.expect_verbose(
//         TokenKind::Dot,
//         // Template terminology is normal here?
//         "Expected a '.' to reference a template, found ",
//         "",
//         None,
//         Branch::Nest,
//         interner,
//     )?;
//
//     let name_id = ctx.expect_id_verbose(
//         TokenKind::Id,
//         "Expected a valid referece to a template, found ",
//         "",
//         Branch::Nest,
//         interner,
//     )?;
//
//     let sym_id = SymbolId::new(name_id);
//
//     dbg!(interner.search(sym_id.id as usize));
//
//     //FIX: Can't report unless I do a quick check first. Fixable.
//     //Maybe type enforce types for ids because silent bugs are very possible
//     let type_def_id = sym_table.get_typedef_id(sym_id).ok_or_else(|| {
//         let err_name = interner.search(sym_id.id as usize);
//
//         ctx.report_verbose(
//             &format!("Could not find any symbol in scope for the reference \"{err_name}\""),
//             None,
//             Branch::NestType,
//         );
//
//         Token::Poison
//     })?;
//
//     let template_id = sym_table.get_template_id(type_def_id).ok_or_else(|| {
//         //TODO: Return result and take the result's type to report it
//         let err_name = interner.search(sym_id.id as usize);
//
//         ctx.report_verbose(
//             &format!("The symbol \"{err_name}\" is not defined as a template"),
//             None,
//             Branch::NestType,
//         );
//
//         Token::Poison
//     })?;
//
//     ctx.expect_verbose(
//         TokenKind::OCurlyBracket,
//         "Expected a '{' to define template, found ",
//         "",
//         None,
//         Branch::NestType,
//         interner,
//     )?;
//
//     let mut new_fields: Vec<TypeIdent> = Vec::new();
//
//     let mut err_count = 0;
//     //FIX: Why does this refuse to continue without inner values?
//
//     let repre = sym_table.extract_template(template_id).repre;
//
//     if ctx.peek_kind() == TokenKind::Id {
//         // EOF check needed here since this is technically an instance of a var branch
//         while ctx.peek_kind() != TokenKind::CCurlyBracket && ctx.peek_kind() != TokenKind::EOF {
//             match repre {
//                 Repre::Struct => {
//                     let type_def_res = parse_var_section(ctx, sym_table, interner);
//
//                     if let Ok(type_def) = type_def_res {
//                         let sym_id = type_def.sym_id;
//                         let type_id = sym_table.store_typedef(type_def);
//
//                         sym_table.store_symbol(sym_id, Symbol::Def(type_id));
//
//                         new_fields.push(type_id);
//                     } else {
//                         if err_count > MAX_ERRORS {
//                             break;
//                         }
//
//                         err_count += 1;
//                     }
//                 }
//                 Repre::Enum => {
//                     let variant_res = handle_variant(ctx, interner, sym_table);
//
//                     if let Ok(variant) = variant_res {
//                     } else {
//                         if err_count > MAX_ERRORS {
//                             break;
//                         }
//
//                         err_count += 1;
//                     }
//                 }
//             }
//         }
//         // Why is this skipped in the scenario of .struct {} but fine with .struct {thing: u32}
//         // It's skipping the loop but somehow skipping this very section.
//
//         let template = sym_table.extract_template_mut(template_id);
//
//         for field in new_fields {
//             template.fields.push(field);
//         }
//
//         ctx.expect_verbose(
//             TokenKind::CCurlyBracket,
//             "Expected a '}' to close defined template, found ",
//             "",
//             None,
//             Branch::NestType,
//             interner,
//         )?;
//     }
//
//     Ok(())
// }
//
// fn handle_variant(
//     ctx: &mut Context,
//     interner: &Intern,
//     sym_table: &mut SymbolTable,
// ) -> Result<TypeIdent, Token> {
//     let name_id = ctx.expect_id_verbose(
//         TokenKind::Id,
//         "Expected a valid enum identifier, found ",
//         "",
//         Branch::Searching,
//         interner,
//     )?;
//     //TODO: C| for complex enums? For complex_rules? Maybe?
//
//     dbg!(interner.search(name_id as usize));
//
//     todo!();
// }
//
// fn parse_complex_section(
//     ctx: &mut Context,
//     interner: &Intern,
//     sym_table: &mut SymbolTable,
// ) -> Result<(), Token> {
//     todo!()
// }
