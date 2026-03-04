pub mod context;
pub mod error;
pub mod symbols;

use crate::parser::error::Branch;
use crate::parser::symbols::{Bind, FuncArgs, FuncDef, SymbolTable, TypeDef};
use crate::token::{ActualPrimitives, Repre, Template};
use crate::{
    parser::{context::Context, symbols::Symbol},
    token::{SpannedToken, Token, TokenKind},
};
use common::intern::Intern;
use common::primitives::PrimitiveKeywords;
use common::symbols::{Cond, InnerArgs, SymbolId, TypeIdent};

// May be lower
const MAX_ERRORS: u8 = 3;

pub fn parse(original_text: &[u8], tokens: &Vec<SpannedToken>, interner: &Intern) -> SymbolTable {
    let mut sym_table = SymbolTable::new();

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

                    parse_bind_section(&mut ctx, &mut sym_table, interner).ok();
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
                        if let Token::Id(name_id) = ctx.peek_tok()
                            && interner.is_section(name_id)
                                // Oh my
                            && ctx.peek_ahead(1).token.kind() != TokenKind::Colon
                        {
                            break;
                        }

                        if let Ok(type_def) = parse_var_section(&mut ctx, &mut sym_table, interner)
                        {
                            let sym_id = SymbolId::new(type_def.sym_id.id);

                            let type_id = sym_table.store_typedef(type_def);

                            sym_table.store_symbol(sym_id, Symbol::Def(type_id));
                        }
                    }
                }
                id if id == PrimitiveKeywords::Nest as u32 => {
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

                        parse_nest_section(&mut ctx, interner, &mut sym_table).ok();
                    }
                }
                id if id == PrimitiveKeywords::ComplexRules as u32 => {
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

                        parse_nest_section(&mut ctx, interner, &mut sym_table).ok();
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
        dbg!(sym_table);

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

    sym_table
}

fn parse_bind_section(
    ctx: &mut Context,
    sym_table: &mut SymbolTable,
    interner: &Intern,
) -> Result<(), Token> {
    let name_id = ctx.expect_id_verbose(
        TokenKind::Literal,
        "Expected a string literal within section `bind`, found ",
        "",
        Branch::Bind,
        interner,
    )?;

    let sym_id = SymbolId::new(name_id);

    let symbol = Symbol::Bind(Bind::new(sym_id));

    sym_table.store_symbol(sym_id, symbol);

    Ok(())
}

fn parse_var_section(
    ctx: &mut Context,
    sym_table: &mut SymbolTable,
    interner: &Intern,
) -> Result<TypeDef, Token> {
    let name_id = ctx.expect_id_verbose(
        TokenKind::Id,
        "Expected an identifier to declare a type, found ",
        "",
        Branch::Var,
        interner,
    )?;

    let sym_id = SymbolId::new(name_id);

    // This seems weird...
    // Ignore this naming
    let err_name = interner.search(name_id as usize);
    ctx.expect_verbose(
        TokenKind::Colon,
        &format!("Expected a ':' after identifier \"{err_name}\" to declare a type, found "),
        "",
        None,
        Branch::VarType,
        interner,
    )?;

    let type_res = parse_type(ctx, sym_table, interner);

    let mut conds: Vec<Cond> = Vec::new();
    // This count cannot end the definition since it would prevent arguments from being viewed
    let mut err_count = 0;

    //FIX: Make this stop on first error
    if ctx.peek_kind() == TokenKind::OBracket {
        ctx.advance_tok();

        loop {
            let new_cond = parse_cond(ctx, sym_table, interner);

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

    //WARN: Should be handled a little more understandably maybe? Maybe this is ok?
    let raw_type = type_res.unwrap_or(TypeIdent::new(0));

    //TEST: Everything is poison checked by the err_vec check at the end so this SHOULD be fine
    let type_def = TypeDef::new(sym_id, raw_type, args, conds);

    Ok(type_def)
}

//FIXME: Give ActualType the function instead
// Still need this...
fn parse_type(
    ctx: &mut Context,
    sym_table: &mut SymbolTable,
    interner: &Intern,
) -> Result<TypeIdent, Token> {
    match ctx.peek_tok() {
        Token::Id(id) => match PrimitiveKeywords::from_sym_id(id) {
            Some(p) => match p {
                PrimitiveKeywords::List => {
                    ctx.advance_tok();

                    let ty = parse_array(ctx, sym_table, interner)?;

                    let list = ActualPrimitives::List(ty);

                    let type_id = sym_table.store_primitive(list);

                    Ok(type_id)
                }
                PrimitiveKeywords::Set => {
                    ctx.advance_tok();

                    let ty = parse_array(ctx, sym_table, interner)?;

                    let set = ActualPrimitives::Set(ty);

                    let type_id = sym_table.store_primitive(set);

                    Ok(type_id)
                }
                PrimitiveKeywords::Map => {
                    ctx.advance_tok();

                    let (key, val) = parse_map(ctx, sym_table, interner)?;

                    let map = ActualPrimitives::Map(key, val);

                    let type_id = sym_table.store_primitive(map);

                    Ok(type_id)
                }
                _ => {
                    let prim = ActualPrimitives::try_from(id).or_else(|_| {
                        ctx.advance_tok();

                        let name = interner.search(id as usize);
                        let msg = format!(
                            "Expected compatible type, found primitive \"{name}\", Branch::VarType"
                        );

                        ctx.report_verbose(&msg, None, Branch::VarType);
                        Err(Token::Poison)
                    })?;

                    ctx.advance_tok();

                    let type_id = sym_table.store_primitive(prim);

                    Ok(type_id)
                }
            },
            None => {
                let name = interner.search(id as usize);

                //TODO: Handle more cleanly
                if name == "S"
                    || name == "E" && ctx.peek_ahead(1).token.kind() == TokenKind::VerticalBar
                {
                    ctx.skip(2);

                    // FIX: I don't know what to name this
                    let template_sym_id = ctx.expect_id_verbose(
                        TokenKind::Id,
                        "Expected a valid type template, found ",
                        "",
                        Branch::VarType,
                        interner,
                    )?;

                    //TODO: Find out whether or not enums should exist internally
                    // FIX: Should it be sym or type...
                    let temp_type_id = TypeIdent::new(template_sym_id);

                    let repre = if name == "S" {
                        Repre::Struct
                    } else {
                        Repre::Enum
                    };

                    let template = Template::new(temp_type_id, repre);

                    let type_id = sym_table.store_template(template);

                    return Ok(type_id);
                }

                let msg = format!("Expected a type, found identifier \"{name}\"");
                ctx.advance_tok();

                //TODO: Deeper help in case meant to be structural type, or primitive, probably.
                ctx.report_verbose(&msg, None, Branch::VarType);

                //WARN:
                Err(Token::Poison)
            }
        },
        Token::QuestionMark => {
            ctx.advance_tok();

            let type_id = sym_table.store_primitive(ActualPrimitives::Any(None));

            Ok(type_id)
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
        Token::VerticalBar => {
            ctx.advance_tok();
            // Probably better off with a help option
            ctx.report_verbose(
                "Expected a valid identifier, found '|'",
                Some("Was this meant to be a template? |e.g. \"S|Struct\" OR \"E|Enum\" |"),
                Branch::VarType,
            );
            //WARN:
            Err(Token::Poison)
        }
        t => {
            ctx.advance_tok();

            let fmt_tok = format!("'{}'", t.kind());

            ctx.report_template("a type", &fmt_tok, Branch::VarType);
            //WARN:
            Err(Token::Poison)
        }
    }
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

fn parse_array(
    ctx: &mut Context,
    sym_table: &mut SymbolTable,
    interner: &Intern,
) -> Result<TypeIdent, Token> {
    //TODO: Probably should just separate func for Sets
    ctx.expect_verbose(
        TokenKind::OAngleBracket,
        "A '<' is required for a `List` or `Set` to take in a type, found ",
        "",
        None,
        Branch::VarType,
        interner,
    )
    .ok();

    let type_id = parse_type(ctx, sym_table, interner)?;

    ctx.expect_verbose(
        TokenKind::CAngleBracket,
        "Expected a '>' to close `List` or `Set`, found ",
        "",
        None,
        Branch::VarType,
        interner,
    )
    .ok();

    Ok(type_id)
}

fn parse_map(
    ctx: &mut Context,
    sym_table: &mut SymbolTable,
    interner: &Intern,
) -> Result<(TypeIdent, TypeIdent), Token> {
    // Kinda weird since the type doesn't exist without a '<'
    ctx.expect_verbose(
        TokenKind::OAngleBracket,
        "Expected a '<' after `Map`, found ",
        "",
        None,
        Branch::VarType,
        interner,
    )
    .ok();

    let key = parse_type(ctx, sym_table, interner)?;

    ctx.expect_verbose(
        TokenKind::Comma,
        "Expecpted a ',' to separate types within `Map`, found ",
        "",
        None,
        Branch::VarType,
        interner,
    )?;

    let val = parse_type(ctx, sym_table, interner)?;

    ctx.expect_verbose(
        TokenKind::CAngleBracket,
        "Expected a '>' to close `Map`, found ",
        "",
        None,
        Branch::VarType,
        interner,
    )
    .ok();

    Ok((key, val))
}

fn parse_cond(
    ctx: &mut Context,
    sym_table: &mut SymbolTable,
    interner: &Intern,
) -> Result<Cond, Token> {
    match ctx.peek_tok() {
        Token::Id(id) => {
            match PrimitiveKeywords::from_sym_id(id) {
                //FIX: Will maybe separate keyword types so that this is easier to handle without
                //code repetition.
                Some(prim) => match prim {
                    PrimitiveKeywords::IsEmpty => {
                        ctx.advance_tok();

                        Ok(Cond::IsEmpty)
                    }
                    PrimitiveKeywords::IsWhitespace => {
                        ctx.advance_tok();

                        Ok(Cond::IsWhitespace)
                    }
                    _ => {
                        ctx.advance_tok();

                        let sym_id = SymbolId::new(id);

                        let func_name = interner.search(id as usize);

                        // Return null version instead?
                        let args = handle_func_args(ctx, func_name, interner)?;

                        let type_id = sym_table.store_func(FuncDef::new(sym_id, args));

                        Ok(Cond::Func(type_id))
                        // ctx.advance_tok();
                        //
                        // let msg = format!("Expected a valid condition, found keyword \"{name}\"");
                        // ctx.report_verbose(&msg, None, Branch::VarCond);
                        //
                        // Err(Token::Poison)
                    }
                },
                //FIX:
                None => {
                    ctx.advance_tok();

                    let sym_id = SymbolId::new(id);

                    let func_name = interner.search(id as usize);

                    let args = handle_func_args(ctx, func_name, interner)?;

                    let type_id = sym_table.store_func(FuncDef::new(sym_id, args));

                    Ok(Cond::Func(type_id))
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

            let wrapped = parse_cond(ctx, sym_table, interner)?;
            dbg!(&wrapped);
            Ok(Cond::Not(Box::new(wrapped)))
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
) -> Result<Vec<FuncArgs>, Token> {
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

    let mut args: Vec<FuncArgs> = Vec::new();

    //FIX:
    loop {
        match ctx.peek_tok() {
            Token::Id(id) => {
                ctx.advance_tok();
                args.push(FuncArgs::Id(SymbolId::new(id)));
            }
            Token::Literal(id) => {
                ctx.advance_tok();
                args.push(FuncArgs::Literal(SymbolId::new(id)));
            }
            Token::Number(id) => {
                ctx.advance_tok();

                let num: usize = interner.search(id as usize).parse().expect("Lexer broke");

                if ctx.peek_kind() == TokenKind::DotRange {
                    ctx.advance_tok();

                    let end = ctx.expect_id_verbose(
                        TokenKind::Number,
                        "Expected a number after (range), found",
                        "",
                        Branch::VarFuncArgs,
                        interner,
                    )?;

                    let end = interner.search(end as usize).parse().expect("Lexer broke");

                    args.push(FuncArgs::Range(num, end));
                } else {
                    args.push(FuncArgs::Num(num));
                }
            }
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

        //WARN: SAFEGUARD
        // if ctx.peek_kind() == TokenKind::CBracket {
        //     return Err(Token::Poison);
        // }

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

fn parse_nest_section(
    ctx: &mut Context,
    interner: &Intern,
    sym_table: &mut SymbolTable,
) -> Result<(), Token> {
    ctx.expect_verbose(
        TokenKind::Dot,
        // Template terminology is normal here?
        "Expected a '.' to reference a template, found ",
        "",
        None,
        Branch::Nest,
        interner,
    )?;

    let name_id = ctx.expect_id_verbose(
        TokenKind::Id,
        "Expected a valid referece to a template, found ",
        "",
        Branch::Nest,
        interner,
    )?;

    let sym_id = SymbolId::new(name_id);

    dbg!(interner.search(sym_id.id as usize));

    //FIX: Can't report unless I do a quick check first. Fixable.
    //Maybe type enforce types for ids because silent bugs are very possible
    let type_def_id = sym_table.get_typedef_id(sym_id).ok_or_else(|| {
        let err_name = interner.search(sym_id.id as usize);

        ctx.report_verbose(
            &format!("Could not find any symbol in scope for the reference \"{err_name}\""),
            None,
            Branch::NestType,
        );

        Token::Poison
    })?;

    let template_id = sym_table.get_template_id(type_def_id).ok_or_else(|| {
        //TODO: Return result and take the result's type to report it
        let err_name = interner.search(sym_id.id as usize);

        ctx.report_verbose(
            &format!("The symbol \"{err_name}\" is not defined as a template"),
            None,
            Branch::NestType,
        );

        Token::Poison
    })?;

    ctx.expect_verbose(
        TokenKind::OCurlyBracket,
        "Expected a '{' to define template, found ",
        "",
        None,
        Branch::NestType,
        interner,
    )?;

    let mut new_fields: Vec<TypeIdent> = Vec::new();

    let mut err_count = 0;
    //FIX: Why does this refuse to continue without inner values?

    if ctx.peek_kind() == TokenKind::Id {
        // EOF check needed here since this is technically an instance of a var branch
        while ctx.peek_kind() != TokenKind::CCurlyBracket && ctx.peek_kind() != TokenKind::EOF {
            let type_def_res = parse_var_section(ctx, sym_table, interner);

            if let Ok(type_def) = type_def_res {
                let sym_id = type_def.sym_id;
                let type_id = sym_table.store_typedef(type_def);

                sym_table.store_symbol(sym_id, Symbol::Def(type_id));

                new_fields.push(type_id);
            } else {
                if err_count > MAX_ERRORS {
                    break;
                }

                err_count += 1;
            }
        }
        // Why is this skipped in the scenario of .struct {} but fine with .struct {thing: u32}
        // It's skipping the loop but somehow skipping this very section.

        let template = sym_table.extract_template_mut(template_id);

        for field in new_fields {
            template.fields.push(field);
        }

        ctx.expect_verbose(
            TokenKind::CCurlyBracket,
            "Expected a '}' to close defined template, found ",
            "",
            None,
            Branch::NestType,
            interner,
        )?;
    }

    Ok(())
}

fn parse_complex_section(
    ctx: &mut Context,
    interner: &Intern,
    sym_table: &mut SymbolTable,
) -> Result<(), Token> {
    todo!()
}
