pub mod context;
pub mod error;
pub mod symbols;

use crate::parser::error::Branch;
use crate::parser::symbols::{Bind, Cond, FuncArgs, FuncDef, InnerArgs, SymbolTable, TypeDef};
use crate::token::{ActualPrimitives, Template};
use crate::{
    parser::{context::Context, symbols::Symbol},
    token::{SpannedToken, Token, TokenKind},
};
use common::intern::Intern;
use common::primitives::PrimitiveKeywords;
use common::symbols::{SymbolId, TypeIdent};

// May be lower
const MAX_ERRORS: u8 = 5;

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
                        Some("Change to \"complex_rules->\""),
                        Branch::Searching,
                        interner,
                    )
                    .ok();
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

        eprintln!("Reported {} errors\n", ctx.err_vec.len());
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
        dbg!(ctx.peek_kind());

        ctx.expect_verbose(
            TokenKind::CBracket,
            "Expected ']' at end of condition, found ",
            "",
            None,
            // Does this set align properly?
            Branch::VarTypeArgs,
            interner,
        )
        .ok();
    }

    let mut args: Vec<InnerArgs> = Vec::new();

    let mut err_count = 0;

    while ctx.peek_kind() == TokenKind::HashSymbol {
        ctx.advance_tok();
        // WARN: Single if check destroys performance.
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
    dbg!(&type_def);

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
        Token::Id(id) => match PrimitiveKeywords::from_id(id) {
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

                // Maybe expect this?
                if name == "S"
                    || name == "E" && ctx.peek_ahead(1).token.kind() == TokenKind::VerticalBar
                {
                    ctx.skip(2);

                    let struct_id = ctx.expect_id_verbose(
                        TokenKind::Id,
                        "Expected a valid type template, found ",
                        "",
                        Branch::VarType,
                        interner,
                    )?;

                    //TODO: Find out whether or not enums should exist internally
                    // FIX:
                    let temp_type_id = TypeIdent::new(struct_id);

                    let template = Template::new(temp_type_id);

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
        // Token::SlimArrow => todo!(),
        // Token::HashSymbol => todo!(),
        // Token::Percent => todo!(),
        // Token::Colon => todo!(),
        // Token::ExclamationPoint => todo!(),
        // Token::Asterisk => todo!(),
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

    // let key = key_res.or_else(|_| Err(Token::Poison))?;
    // let val = val_res.or_else(|_| Err(Token::Poison))?;

    Ok((key, val))
}

fn parse_cond(
    ctx: &mut Context,
    sym_table: &mut SymbolTable,
    interner: &Intern,
) -> Result<Cond, Token> {
    match ctx.peek_tok() {
        Token::Id(id) => {
            let name = interner.search(id as usize);

            match PrimitiveKeywords::from_id(id) {
                Some(prim) => match prim {
                    PrimitiveKeywords::Range => {
                        ctx.advance_tok();

                        let sym_id = SymbolId::new(id);
                        let args = handle_func_args(ctx, interner)?;

                        let type_id = sym_table.store_func(FuncDef::new(sym_id, args));

                        Ok(Cond::Func(type_id))
                    }
                    PrimitiveKeywords::IsEmpty => {
                        ctx.advance_tok();
                        // TODO: Ok this needs to be fixed now.

                        Ok(Cond::IsEmpty)
                    }
                    _ => {
                        // Function similar check?
                        ctx.advance_tok();

                        let msg = format!("Expected a valid condition, found \"{name}\"");
                        ctx.report_verbose(&msg, None, Branch::VarCond);

                        Err(Token::Poison)
                    }
                },
                None => {
                    unimplemented!("No custom functions");
                    // Cond::Function(FunctionDef::new(id)),
                }
            }
            // // Notations
            // n => {
            //     let fmt_tok = format!("\"{n}\"");
            //     ctx.report_template(
            //         "a condition after declared type",
            //         &fmt_tok,
            //         Branch::VarCond,
            //     );
            //
            //     //WARN:
            //     Err(Token::Poison)
            // }
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

//TODO: Should not explicitly be processed here. More general function parser.
fn handle_func_args(ctx: &mut Context, interner: &Intern) -> Result<Vec<FuncArgs>, Token> {
    todo!();
    // ctx.expect_verbose(
    //     TokenKind::OParen,
    //     "Missing '(' within function `Range()`, found ",
    //     "",
    //     None,
    //     Branch::VarCond,
    //     interner,
    // )
    // .ok();
    //
    // let mut start: usize = 0;
    //
    // let end_id = match ctx.peek_tok() {
    //     Token::Tilde => {
    //         ctx.advance_tok();
    //
    //         ctx.expect_id_verbose(
    //             TokenKind::Number,
    //             "Expected a number after '~', found ",
    //             " within `Len()`. Use '(~x1)' or '(x1..=x2)' to define a range.",
    //             Branch::VarCond,
    //             interner,
    //         )?
    //     }
    //     Token::Number(id) => {
    //         ctx.advance_tok();
    //         let raw_num = interner.search(id as usize);
    //
    //         start = match raw_num.parse::<usize>() {
    //             Ok(n) => n,
    //             Err(_) => {
    //                 panic!("[temp] Internal error. Failed to parse number in condition.");
    //                 // ctx.report_template(emsg, fmsg, branch);
    //                 // return Err(ctx.advance().token);
    //             }
    //         };
    //
    //         ctx.expect_verbose(
    //             TokenKind::DotRange,
    //             &format!("Missing rest of (range) after the number {raw_num}, found "),
    //             "",
    //             Some("Use 'Len(~x1)' or 'Len(x1..=x2)' to define a range."),
    //             Branch::VarCond,
    //             interner,
    //         )?;
    //
    //         //WARN: Hard coded help
    //         let user_help = format!(". |e.g. {start}..=other|");
    //
    //         ctx.expect_id_verbose(
    //             TokenKind::Number,
    //             "Expected a number at the end of (range), found ",
    //             &user_help,
    //             Branch::VarCond,
    //             interner,
    //         )?
    //     }
    //     Token::Id(id) | Token::Literal(id) => {
    //         let err_tok = ctx.advance_tok();
    //         let name = interner.search(id as usize);
    //
    //         let fmt_tok = format!("{} \"{}\" while parsing condition.", err_tok.kind(), name);
    //
    //         ctx.report_template("a (range) or number", &fmt_tok, Branch::VarCond);
    //         return Err(err_tok);
    //     }
    //     t => {
    //         ctx.advance_tok();
    //         let fmt_tok = format!("'{}'", t.kind());
    //
    //         // TODO: How would a range be hinted here?
    //         ctx.report_template(
    //             "a valid (range) or number in parameters",
    //             &fmt_tok,
    //             Branch::VarCond,
    //         );
    //         return Err(t);
    //     }
    // };
    //
    // let end = interner.search(end_id as usize);
    //
    // let end = end
    //     .parse()
    //     //TODO: report this
    //     .or_else(|_| {
    //         let msg = format!(
    //             "[Internal] Failed to parse value '{end}'. Although shouldn't be possible."
    //         );
    //
    //         ctx.report_verbose(&msg, None, Branch::VarCond);
    //         //WARN:
    //         return Err(Token::Poison);
    //     })?;
    //
    // if start > end {
    //     let msg = format!("The range '{start}..={end}' is invalid. Cannot have end > start.");
    //     ctx.report_verbose(&msg, None, Branch::VarCond);
    // }
    //
    // ctx.expect_verbose(
    //     TokenKind::CParen,
    //     "Expected ')' after `Len()`, found ",
    //     "",
    //     Some("Individual type conditions must be closed with ')' or delimited by commas |e.g. `(Len(~4)` |"),
    //     Branch::VarCond,
    //     interner,
    // )
    // .ok();
    //
    // ctx.exit_if(Branch::VarType)?;
    //
    // Ok(vec![FuncArgs::Num(start), FuncArgs::Num(end)])
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
    )
    .ok();

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
            &format!("Could not find any type defined for the reference \"{err_name}\""),
            None,
            Branch::NestType,
        );

        Token::Poison
    })?;

    //FIX: Can't option this either because report would call recover, but recover would skip past
    //the curly bracket, meaning everything would be hallucinated onwards
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

        dbg!(interner.search(sym_id.id as usize));
        dbg!(sym_id);

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
) -> Result<Symbol, Token> {
    todo!()
}
