pub mod ast;
mod branch;
mod context;
mod evidence;
mod parse_fmt;
mod parser_budget;
mod parser_state;

use crate::lexer::token::{SpannedToken, Token, TokenKind};
use crate::lookup::scopes::{ScopeLookupPattern, ScopeType};
use crate::parser::ast::ast_concepts::{
    AbstractAlias, AbstractConfig, AbstractConfigKind, AbstractDirective, AbstractEnum,
    AbstractMemberAccess, AbstractOptionAssignment, AbstractParam, AbstractStruct, AbstractTypeDef,
    AbstractVar, AbstractVariant, AstInfo, BinaryOp, Item, SectionKind, Unary, UnaryOp,
};

use crate::parser::ast::ast_exprs::{
    ArrayExpr, Expr, Generic, PathSegment, SpannedExpr, SpannedPathSegment, TypeExpr,
};
use crate::parser::branch::{Branch, NestBranch, NeutralBranch, SectionBranch};
use crate::parser::context::ParserContext;
use crate::parser::parser_budget::ParserBudget;
use crate::parser::parser_state::ParserState;
use crate::semantic::hir::hir_concepts::{
    ComplexConfigMemberMetadata, ConfigMemberMetadataKind, OverrideConfigMemberMetadata,
};
use chrn_utils::chrn_config::ChrnConfig;
use chrn_utils::id_types::{InternedId, SpannedContainer};
use chrn_utils::intern::Intern;
use chrn_utils::source_map::source_diagnostic::{SourceDiagnostic, SourceDiagnosticSummary};
use chrn_utils::source_map::source_region::SourceRegion;
use chrn_utils::source_map::source_span::SourceSpan;
use lang::fmter::{Formattable, Formatted};
use lang::keywords::Keyword;

// The CST.
/// Returns a tuple of `AstInfo` and Diagnostics, where `AstInfo` may or may not be unfinished,
/// depending on if diagnostics > 0
pub fn parse(
    cfg: &ChrnConfig,
    region: &SourceRegion,
    tokens: &[SpannedToken],
    interner: &Intern,
) -> (AstInfo, SourceDiagnosticSummary) {
    // Output it's own summary? Does AstInfo hold a summary?
    let mut ast_info = AstInfo::new();

    let mut state = ParserState::new();
    let budget = ParserBudget::new(
        chrn_utils::MAX_RECURSIVE_DEPTH,
        chrn_utils::MAX_EXPR_NODES as usize,
    );
    let mut ctx = ParserContext::new(cfg, region, tokens);

    // Skipping possible @def first since it is recognized as it's own token
    if ctx.peek_tok() == Token::Def {
        ctx.advance_tok();
    }

    while !ctx.peek_kind().is_terminator() {
        // Checks if there is an export which is only a boolean due to there only being private and
        // public
        let is_priv = match parse_export(&mut ctx, interner) {
            Ok(b) => b,
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
                            "Duplicate bind statement",
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

                    if let Ok(abs_alias) = parse_alias_stmt(&mut ctx, is_priv, &budget, interner) {
                        let item = Item::Alias(abs_alias);
                        ast_info.push_item(SectionKind::Neutral, item);
                    };
                }
                Keyword::Let => {
                    ctx.advance_tok();

                    if let Ok(abs_var) = parse_let(&mut ctx, is_priv, &budget, interner) {
                        let item = Item::Var(abs_var);
                        ast_info.push_item(SectionKind::Neutral, item);
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
                            "Duplicate `var` section",
                            Branch::Section(SectionBranch::Searching),
                            interner,
                        );

                        continue;
                    } else {
                        // For metadata purposes in case it's empty and we still need to know if it was
                        // used or not
                        state.flip_var();
                        ast_info.push_sect(SectionKind::Var);
                    }

                    _ = ctx.expect_verbose(
                        TokenKind::SlimArrow,
                        "Expected '->' after section `var`, found ",
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

                        if let Ok(type_def) = parse_typedef(&mut ctx, &budget, interner) {
                            let item = Item::TypeDef(type_def);
                            ast_info.push_item(SectionKind::Var, item);
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
                            "Duplicate `nest` section",
                            Branch::Section(SectionBranch::Searching),
                            interner,
                        );
                        continue;
                    } else {
                        state.flip_nest();
                        ast_info.push_sect(SectionKind::Nest);
                    }

                    _ = ctx.expect_verbose(
                        TokenKind::SlimArrow,
                        "Expected '->' after section `nest`, found ",
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

                        if let Ok(item) = parse_nest_sect(&mut ctx, is_priv, &budget, interner) {
                            ast_info.push_item(SectionKind::Nest, item);
                        }
                    }
                }
                Keyword::Complex => {
                    if !is_priv {
                        report_export(&mut ctx, Formatted::SectNest, Branch::Searching, interner);
                    }

                    ctx.advance_tok();

                    //TODO: Maybe this shouldn't continue since if complex was used twice that
                    //probably means syntax should at least still be shown
                    if state.has_complex() {
                        ctx.report_verbose(
                            "Duplicate `complex` section",
                            Branch::Section(SectionBranch::Searching),
                            interner,
                        );
                        continue;
                    } else {
                        state.flip_complex();
                        ast_info.push_sect(SectionKind::Complex);
                    }

                    _ = ctx.expect_verbose(
                        TokenKind::SlimArrow,
                        "Expected '->' after section `complex`, found ",
                        "",
                        Branch::Section(SectionBranch::Searching),
                        interner,
                    );

                    while !ctx.peek_kind().is_terminator() {
                        if let Token::Keyword(kw) = ctx.peek_tok()
                            && kw.is_sect()
                                // Deciding to use `in` just because it conflicts with breaks seems
                                // like a flaw internally where var SHOULD be able to be used
                                // outside of sections, even if it is integral...or at least
                                // probably should be able to
                            && ctx.peek_ahead(1).tok == Token::SlimArrow
                        {
                            break;
                        }

                        if let Ok(abs_cfg) =
                            parse_cfg_expr(&mut ctx, &budget, true, ScopeType::Complex, interner)
                        {
                            ast_info.push_item(SectionKind::Complex, Item::Config(abs_cfg));
                        }
                    }
                }
                Keyword::Override => {
                    if !is_priv {
                        report_export(
                            &mut ctx,
                            Formatted::SectComplex,
                            Branch::Section(SectionBranch::Searching),
                            interner,
                        );
                    }

                    ctx.advance_tok();

                    if state.has_override() {
                        ctx.report_verbose(
                            "Duplicate `override` section",
                            Branch::Section(SectionBranch::Searching),
                            interner,
                        );
                        continue;
                    } else {
                        state.flip_override();
                        ast_info.push_sect(SectionKind::Override);
                    }

                    _ = ctx.expect_verbose(
                        TokenKind::SlimArrow,
                        "Expected '->' after section `override`, found ",
                        "",
                        Branch::Section(SectionBranch::Searching),
                        interner,
                    );
                    // Please lint empty sections please emit 40000 warns for slightly misplaced
                    // spaces
                    // As you wish.

                    while !ctx.peek_kind().is_terminator() {
                        // This would look simpler with keywords
                        if let Token::Keyword(kw) = ctx.peek_tok()
                            && kw.is_sect()
                            && ctx.peek_ahead(1).tok == Token::SlimArrow
                        {
                            break;
                        }

                        if let Ok(abs_cfg) =
                            parse_cfg_expr(&mut ctx, &budget, true, ScopeType::Override, interner)
                        {
                            ast_info.push_item(SectionKind::Override, Item::Config(abs_cfg));
                        }
                    }
                }
                _ => {
                    // kw
                    ctx.advance_tok();

                    if state.is_neutral() {
                        ctx.report_template(
                            "a statement or section",
                            &parse_fmt::fmt_tok(tok, interner),
                            Branch::Neutral(NeutralBranch::Searching),
                            interner,
                        );
                    } else {
                        ctx.report_template(
                            "a section with '->' after",
                            &parse_fmt::fmt_tok(tok, interner),
                            Branch::Section(SectionBranch::Searching),
                            interner,
                        );
                    }
                }
            },
            Token::Invalid(id) => {
                ctx.advance_tok();
                let err_str = interner.search(id);
                let msg = format!("Invalid token {err_str}");

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

                let fmsg = parse_fmt::fmt_tok(t, interner);
                ctx.advance_tok();
                ctx.report_template(allowed_msg, &fmsg, Branch::Searching, interner);
            }
        }
    }

    // Returning broken ast and the diagnostics

    (ast_info, ctx.summary)
}

//FIXME: These sets may be misaligned
fn parse_alias_stmt(
    ctx: &mut ParserContext,
    is_priv: bool,
    budget: &ParserBudget,
    interner: &Intern,
) -> Result<AbstractAlias, Token> {
    let name_span = ctx.peek_span();

    let name_id = ctx.expect_id_verbose(
        TokenKind::Id,
        "Expected identifier after `alias`, found ",
        "",
        Branch::Neutral(NeutralBranch::Alias),
        interner,
    )?;

    ctx.expect_verbose(
        TokenKind::OParen,
        "Expected parameters to define alias, found ",
        "",
        Branch::Neutral(NeutralBranch::Alias),
        interner,
    )?;

    let params = parse_alias_decl(ctx, budget, interner)?;

    ctx.expect_verbose(
        TokenKind::Assign,
        "Expected '=' to define alias, found ",
        "",
        Branch::Neutral(NeutralBranch::Alias),
        interner,
    )?;

    let conds = if ctx.peek_kind() == TokenKind::OBracket {
        handle_conds(ctx, budget, interner).unwrap_or_default()
    } else {
        Vec::new()
    };

    let directives = if ctx.peek_kind() == TokenKind::HashSymbol {
        handle_directives(ctx, interner).unwrap_or_default()
    } else {
        Vec::new()
    };

    // TODO: The case of "alias x() = " should maybe be an error..?
    // if conds.is_empty() && args.is_empty() {
    //     ctx.report_verbose("", Branch::Neutral(NeutralBranch::Alias), interner);
    // }

    let alias = AbstractAlias::new(name_id, name_span, params, conds, directives, is_priv);

    Ok(alias)
}

/// Since `modules.rs` handles modules with it's own mini-parser, this is just a parser check.
fn check_bind(ctx: &mut ParserContext, interner: &Intern) -> Result<(), Token> {
    ctx.expect_id_verbose(
        TokenKind::Str,
        "Expected string literal after `bind`, found ",
        "",
        Branch::Neutral(NeutralBranch::Bind),
        interner,
    )?;

    Ok(())
}

/// Since `modules.rs` handles modules with it's own mini-parser, this is just a parser check.
fn check_import(ctx: &mut ParserContext, interner: &Intern) -> Result<(), Token> {
    ctx.expect_id_verbose(
        TokenKind::Str,
        "Expected string literal path, found ",
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
            "Expected import alias after `as`, found ",
            "",
            Branch::Neutral(NeutralBranch::Import),
            interner,
        )?;
    }

    Ok(())
}

fn parse_typedef(
    ctx: &mut ParserContext,
    budget: &ParserBudget,
    interner: &Intern,
) -> Result<AbstractTypeDef, Token> {
    let name_span = ctx.peek_span();

    let name_id = ctx.expect_id_verbose(
        TokenKind::Id,
        "Expected identifier to define type, found ",
        "",
        // Not exactly true that var is being used, just that the type definition flow already
        // exists, so...
        Branch::Section(SectionBranch::Var),
        interner,
    )?;

    let err_name = interner.search(name_id);

    ctx.expect_verbose(
        TokenKind::Colon,
        &format!("Expected a ':' after `{err_name}` to define a type, found "),
        "",
        Branch::Section(SectionBranch::Var),
        interner,
    )?;

    let ty = parse_type_expr(ctx, budget, interner)?;

    let conds = if ctx.peek_kind() == TokenKind::OBracket {
        handle_conds(ctx, budget, interner).unwrap_or_default()
    } else {
        Vec::new()
    };

    let directives = if ctx.peek_kind() == TokenKind::HashSymbol {
        handle_directives(ctx, interner).unwrap_or_default()
    } else {
        Vec::new()
    };

    if ctx.peek_kind() == TokenKind::Comma {
        ctx.advance_tok();
    }

    let abs_typedef = AbstractTypeDef::new(name_id, name_span, ty, directives, conds);

    Ok(abs_typedef)
}

fn parse_nest_sect(
    ctx: &mut ParserContext,
    is_priv: bool,
    budget: &ParserBudget,
    interner: &Intern,
) -> Result<Item, Token> {
    // Wait what is this error?
    let kw = ctx.expect_kw_verbose(
        "Expected `enum` or `struct`, found ",
        "",
        Branch::Section(NestBranch::NestStart.into()),
        interner,
    )?;

    //TODO: Can likely be done simpler but keep for simplicity
    let item = match kw {
        Keyword::Struct => {
            let name_span = ctx.peek_span();

            let name_id = ctx.expect_id_verbose(
                TokenKind::Id,
                "Expected identifier for struct, found ",
                "",
                Branch::Section(NestBranch::StructType.into()),
                interner,
            )?;

            let struct_name = interner.search(name_id);

            ctx.expect_verbose(
                TokenKind::OCurlyBracket,
                &format!("Expected '{{' to define struct, found "),
                "",
                Branch::Section(NestBranch::StructType.into()),
                interner,
            )?;

            let fields =
                handle_struct_fields(ctx, struct_name, budget, interner).unwrap_or_default();

            let conds = if ctx.peek_kind() == TokenKind::OBracket {
                // Uses unwrap_or_default() in many places so that the rest can be parsed if present for
                // better errors
                handle_conds(ctx, budget, interner).unwrap_or_default()
            } else {
                Vec::new()
            };

            let args = if ctx.peek_kind() == TokenKind::HashSymbol {
                handle_directives(ctx, interner).unwrap_or_default()
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

            let name_id = ctx.expect_id_verbose(
                TokenKind::Id,
                "Expected identifier for enum, found ",
                "",
                Branch::Section(NestBranch::EnumType.into()),
                interner,
            )?;

            let enum_name = interner.search(name_id);

            ctx.expect_verbose(
                TokenKind::OCurlyBracket,
                &format!("Expected '{{' to define enum, found"),
                "",
                Branch::Section(NestBranch::EnumType.into()),
                interner,
            )?;

            let variants = handle_enum_variants(ctx, enum_name, budget, interner)?;

            let glob_conds = if ctx.peek_kind() == TokenKind::OBracket {
                handle_conds(ctx, budget, interner).unwrap_or_default()
            } else {
                Vec::new()
            };

            let glob_directives = if ctx.peek_kind() == TokenKind::HashSymbol {
                handle_directives(ctx, interner).unwrap_or_default()
            } else {
                Vec::new()
            };

            let enumeration = AbstractEnum::new(
                name_id,
                name_span,
                variants,
                glob_conds,
                glob_directives,
                is_priv,
            );

            Item::Enum(enumeration)
        }
        _ => {
            ctx.report_verbose(
                &format!(
                    "Expected keyword `enum` or `struct`, found `{}`",
                    kw.to_fmt()
                ),
                Branch::Section(NestBranch::NestStart.into()),
                interner,
            );

            return Err(Token::Poison);
        }
    };

    Ok(item)
}

//TODO: Better complex branching tracking so that help messages can be made
//
// Maybe, separate config and override structures. Maybe.
// We'll see :(
fn parse_cfg_expr(
    ctx: &mut ParserContext,
    budget: &ParserBudget,
    // It's only one depth so just reflecting it with one T/F state
    is_root: bool,
    scope_type: ScopeType,
    interner: &Intern,
) -> Result<AbstractConfig, Token> {
    let _guard = budget.increase_depth();
    // Oh wow this looks great
    //
    // If the prefix is something like "var x {}" then it for this special case allows for another
    // section to lookup var
    let (lookup_pat, name_id, name_span, kind) =
        handle_cfg_metadata(ctx, is_root, scope_type, interner)?;

    // Allows for "=>" to notify that
    if ctx.peek_tok() != Token::OCurlyBracket && ctx.peek_tok() != Token::NotSlimArrow {
        ctx.report_verbose(
            "Expected '{' block or '=>' to define config, found ",
            Branch::Section(SectionBranch::Complex),
            interner,
        );
        return Err(Token::Poison);
    }

    // Catching state1=>state2=>state3 {} syntax SHORTENING (Different)
    //
    // Is ok to advance() since last check ensures that it must be either an arrow or CCurly, but we
    // only care about the arrow here
    let used_arrow = ctx.advance_tok() == Token::NotSlimArrow;

    let mut opt_assignments: Vec<AbstractOptionAssignment> = Vec::new();
    let mut cfg_members: Vec<AbstractConfig> = Vec::new();

    loop {
        if ctx.peek_ahead(1).tok == Token::Assign {
            // for "cases = [snake_case]"
            match parse_option_assignment(ctx, budget, interner) {
                //WARN: The attempt to make recovery better doesn't really work since it would
                //basically need to propagate the fact that it wants to skip looking for
                //the closing '}' for an invalid config by just breaking for all of them.
                //So, this may return a should_break.
                Ok(opt) => opt_assignments.push(opt),
                // So the parsing details aren't lost
                Err(_) => break,
            };
            // opt_assignments.push(opt_assignment);
        } else if ctx.peek_kind() == TokenKind::Id
            // "nest/override Type {}" can be used so we need to catch those semantic identifiers
            || ctx.peek_tok() == Token::Keyword(Keyword::Override)
            || ctx.peek_tok() == Token::Keyword(Keyword::Nest)
        {
            // for "inner {/*assignments*/}"
            match parse_cfg_expr(ctx, budget, false, scope_type, interner) {
                Ok(abs_cfg) => cfg_members.push(abs_cfg),
                Err(_) => break,
            };
            // cfg_members.push(abs_cfg_member);
        } else {
            // If no consumable token for this branch is seen
            //
            // To prevent looking for matching "}" when "=>" syntax was used
            if !used_arrow {
                ctx.expect_verbose(
                    TokenKind::CCurlyBracket,
                    "Expected '}' or more declarations, found ",
                    "",
                    Branch::Section(SectionBranch::Complex),
                    interner,
                )?;
            }

            break;
        }
    }

    // Might have to separate these, parent and member.
    Ok(AbstractConfig::new(
        name_id,
        name_span,
        kind,
        lookup_pat,
        opt_assignments,
        cfg_members,
    ))
}

/// Helper for handling metadata for the different semantic versions of a config.
/// The config can be, a root, complex( with name or with semantic override meaning ),
/// override (Not done yet)
///
/// On `Ok`: Returns (ADDRESS ME)
fn handle_cfg_metadata(
    ctx: &mut ParserContext,
    is_root: bool,
    scope_type: ScopeType,
    interner: &Intern,
) -> Result<
    // I KNOW THIS LOOKS BAD. WAIT.
    (
        ScopeLookupPattern,
        InternedId,
        SourceSpan,
        AbstractConfigKind,
    ),
    Token,
> {
    //TODO: Collapse these
    match scope_type {
        ScopeType::Complex => {
            if is_root {
                // Only keywords valid for root usage
                let pat = if ctx.peek_tok() == Token::Keyword(Keyword::Var) {
                    ctx.advance_tok();
                    ScopeLookupPattern::OnlyVar
                } else if ctx.peek_tok() == Token::Keyword(Keyword::Nest) {
                    ctx.advance_tok();
                    ScopeLookupPattern::OnlyNest
                } else {
                    ScopeLookupPattern::NamespaceOnly
                };

                let name_span = ctx.peek_span();
                let name_id = ctx.expect_id_verbose(
                    TokenKind::Id,
                    "Expected identifier to define config, found ",
                    "",
                    Branch::Section(SectionBranch::Complex),
                    interner,
                )?;
                return Ok((pat, name_id, name_span, AbstractConfigKind::Root));
            } else {
                // If !root
                let name_span = ctx.peek_span();
                let name_id = ctx.expect_id_verbose(
                    TokenKind::Id,
                    "Expected identifier to define config, found ",
                    "",
                    Branch::Section(SectionBranch::Complex),
                    interner,
                )?;

                let kind = AbstractConfigKind::Member(ConfigMemberMetadataKind::Complex(
                    ComplexConfigMemberMetadata::new(),
                ));

                return Ok((ScopeLookupPattern::NamespaceOnly, name_id, name_span, kind));
            }
        }
        ScopeType::Override => {
            if is_root {
                // Only keywords valid for root usage
                let pat = if ctx.peek_tok() == Token::Keyword(Keyword::Var) {
                    ctx.advance_tok();
                    ScopeLookupPattern::OnlyVar
                } else if ctx.peek_tok() == Token::Keyword(Keyword::Nest) {
                    ctx.advance_tok();
                    ScopeLookupPattern::OnlyNest
                } else {
                    ScopeLookupPattern::NamespaceOnly
                };

                let name_span = ctx.peek_span();
                let name_id = ctx.expect_id_verbose(
                    TokenKind::Id,
                    "Expected an identifier to define config, found ",
                    "",
                    Branch::Section(SectionBranch::Override),
                    interner,
                )?;
                return Ok((pat, name_id, name_span, AbstractConfigKind::Root));
            } else {
                // If !root
                let name_span = ctx.peek_span();
                let name_id = ctx.expect_id_verbose(
                    TokenKind::Id,
                    "Expected identifier to define config, found ",
                    "",
                    Branch::Section(SectionBranch::Complex),
                    interner,
                )?;

                let kind = AbstractConfigKind::Member(ConfigMemberMetadataKind::Override(
                    OverrideConfigMemberMetadata::new(),
                ));

                return Ok((ScopeLookupPattern::NamespaceOnly, name_id, name_span, kind));
            }
        }
        _ => unreachable!(),
    }
}

// The field assignments could be ANYWHERE as long as it's in a valid scope with the parent it's
// defining, so it's probably best to just keep this one by one in control flow to avoid allocating
// and appending vecs just because a field assignment was after a nested config.
fn parse_option_assignment(
    ctx: &mut ParserContext,
    budget: &ParserBudget,
    interner: &Intern,
) -> Result<AbstractOptionAssignment, Token> {
    let name_span = ctx.peek_span();

    let name_id = ctx.expect_id_verbose(
        TokenKind::Id,
        "Expected identifier for config option, found ",
        "",
        Branch::Section(SectionBranch::Complex),
        interner,
    )?;

    ctx.expect_verbose(
        TokenKind::Assign,
        "Expected '=' to assign value to option, found ",
        "",
        Branch::Section(SectionBranch::Complex),
        interner,
    )?;

    let sp_array_expr = if ctx.peek_tok() == Token::OBracket {
        parse_array(ctx, budget, interner)?
    } else {
        // Assumes it's a single value assignment if no OBracket is present
        let only_element = parse_expr(ctx, 0, budget, interner)?;
        let span = only_element.span;
        let array_expr = Expr::Array(ArrayExpr::new(vec![only_element]));

        SpannedExpr::new(array_expr, span)
    };

    // Allows for trailing comma
    if ctx.peek_tok() == Token::Comma {
        ctx.advance_tok();
    }

    Ok(AbstractOptionAssignment::new(
        name_id,
        name_span,
        sp_array_expr,
    ))
}

// Should this just return elements similar to how call_args does?
//NOTE: May add parse_array to parse_expr eventually
/// Parses assuming that '[' is the starting point.
/// Parses ',' delimited elements "[1,2,3,4,]"
fn parse_array(
    ctx: &mut ParserContext,
    budget: &ParserBudget,
    interner: &Intern,
) -> Result<SpannedExpr, Token> {
    ctx.expect_verbose(
        TokenKind::OBracket,
        "Expected '[' to declare array, found ",
        "",
        Branch::Expr,
        interner,
    )?;

    let mut elements: Vec<SpannedExpr> = Vec::new();

    let start = ctx.peek_span().start;

    while !ctx.peek_tok().kind().is_terminator() && ctx.peek_tok() != Token::CBracket {
        let sp_expr = parse_expr(ctx, 0, budget, interner)?;
        elements.push(sp_expr);

        if ctx.peek_tok() == Token::CBracket {
            break;
        }

        ctx.expect_verbose(
            TokenKind::Comma,
            "Expected a comma to separate elements, found ",
            "",
            Branch::Expr,
            interner,
        )?;
    }

    let end = ctx.peek_span().end;
    let span = SourceSpan::new(ctx.region.region_id, start, end);

    ctx.expect_verbose(
        TokenKind::CBracket,
        "Expected ']' to close array, found ",
        "",
        Branch::Expr,
        interner,
    )?;

    let array_expr = ArrayExpr::new(elements);

    Ok(SpannedExpr::new(Expr::Array(array_expr), span))
}

//TODO:
fn parse_override_sect(ctx: &mut ParserContext, interner: &Intern) -> Result<(), Token> {
    todo!()
}

fn parse_let(
    ctx: &mut ParserContext,
    is_priv: bool,
    budget: &ParserBudget,
    interner: &Intern,
) -> Result<AbstractVar, Token> {
    let name_span = ctx.peek_span();

    let name_id = ctx.expect_id_verbose(
        TokenKind::Id,
        "Expected identifier after `let`, found ",
        "",
        Branch::Neutral(NeutralBranch::Let),
        interner,
    )?;

    ctx.expect_verbose(
        TokenKind::Assign,
        "Expected '=' to declare value, found ",
        "",
        Branch::Neutral(NeutralBranch::Let),
        interner,
    )?;

    let spanned_expr = parse_expr(ctx, 0, budget, interner)?;

    let abs_var = AbstractVar::new(name_id, name_span, spanned_expr, is_priv);

    Ok(abs_var)
}

/// Pratt Parser for all expression kinds except type expressions
fn parse_expr(
    ctx: &mut ParserContext,
    min_bp: u8,
    budget: &ParserBudget,
    interner: &Intern,
) -> Result<SpannedExpr, Token> {
    let _guard = budget.increase_depth().map_err(|_| {
        let msg = format!(
            "Reached max recursive depth of {}",
            budget.recursion_tracker.limit()
        );

        ctx.report_verbose(&msg, Branch::Type, interner);
        Token::Poison
    })?;

    let mut lhs = parse_unary(ctx, budget, interner)?;

    // I REFUSE TO BREAK APART TOKENS
    loop {
        // Use lookahead to detect << and >> as shift operators, avoiding conflict
        // with generic angle brackets in type contexts.
        let is_lshift =
            ctx.peek_tok() == Token::OAngleBracket && ctx.peek_ahead(1).tok == Token::OAngleBracket;
        let is_rshift =
            ctx.peek_tok() == Token::CAngleBracket && ctx.peek_ahead(1).tok == Token::CAngleBracket;

        if is_lshift || is_rshift {
            let bp = 1;
            if bp < min_bp {
                break;
            }

            let start = ctx.peek_span().start;
            ctx.advance_tok();
            ctx.advance_tok();
            let op = if is_lshift {
                BinaryOp::BitLeftShift
            } else {
                BinaryOp::BitRightShift
            };

            let rhs = parse_expr(ctx, bp + 1, budget, interner)?;

            let end = rhs.span.end;
            let span = SourceSpan::new(ctx.region.region_id, start, end);

            lhs = SpannedExpr::new(
                Expr::BinaryExpr {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                },
                span,
            );
        } else if let Some((op, bp)) = ctx.peek_tok().precedence() {
            if bp < min_bp {
                break;
            }

            ctx.advance_tok();

            let rhs = parse_expr(ctx, bp + 1, budget, interner)?;

            let span = SourceSpan::new(ctx.region.region_id, lhs.span.start, rhs.span.end);
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

            let args = parse_call_args(ctx, budget, interner)?;
            let span = SourceSpan::new(
                ctx.region.region_id,
                lhs.span.start,
                ctx.peek_behind(1).span.end,
            );

            lhs = SpannedExpr::new(Expr::Call(Box::new(lhs), args), span);
        } else if ctx.peek_kind() == TokenKind::Id && ctx.peek_ahead(1).tok == Token::OParen {
            let bp = 100;
            if bp < min_bp {
                break;
            }

            let call_start = ctx.advance_span();
            ctx.advance_tok();

            let args = parse_call_args(ctx, budget, interner)?;
            let span = SourceSpan::new(
                ctx.region.region_id,
                call_start.start,
                ctx.peek_behind(1).span.end,
            );

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

            let span = SourceSpan::new(
                ctx.region.region_id,
                lhs.span.start,
                ctx.peek_behind(1).span.end,
            );

            lhs = SpannedExpr::new(
                Expr::MemberAccess(AbstractMemberAccess::new(Box::new(lhs), field_id)),
                span,
            );
        } else {
            break;
        }
    }

    Ok(lhs)
}

// For single values, likley lhs
fn parse_primary(
    ctx: &mut ParserContext,
    budget: &ParserBudget,
    interner: &Intern,
) -> Result<SpannedExpr, Token> {
    //TEST:
    let _guard = budget.increase_depth().map_err(|_| {
        let msg = format!(
            "Reached max recursive depth of {}",
            budget.recursion_tracker.limit()
        );

        ctx.report_verbose(&msg, Branch::Type, interner);
        Token::Poison
    })?;
    match ctx.peek_tok() {
        Token::OParen => {
            ctx.advance_tok();
            let expr = parse_expr(ctx, 0, budget, interner)?;

            ctx.expect_verbose(
                TokenKind::CParen,
                "Expected ')' to close grouped expression, found ",
                "",
                Branch::Expr,
                interner,
            )?;

            Ok(expr)
        }
        Token::Id(name_id) if ctx.peek_ahead(1).tok == Token::Assign => {
            let ident_span = ctx.advance_span();

            let ident_expr = SpannedExpr::new(Expr::Var(name_id), ident_span);

            ctx.advance_tok();

            let expr = parse_expr(ctx, 0, budget, interner)?;

            let span = SourceSpan::new(
                ctx.region.region_id,
                ident_span.start,
                ctx.peek_behind(1).span.end,
            );

            let default = Expr::Default(Box::new(ident_expr), Box::new(expr));

            Ok(SpannedExpr::new(default, span))
        }
        Token::Id(_) if ctx.peek_ahead(1).tok == Token::StaticAccess => {
            let start = ctx.peek_span().start;
            let access_path = parse_static_path(ctx, budget, interner)?;
            let end = ctx.peek_behind(1).span.end;

            let static_span = SourceSpan::new(ctx.region.region_id, start, end);
            let sp_expr = SpannedExpr::new(Expr::StaticAccess(access_path), static_span);

            Ok(sp_expr)
        }
        Token::BoolLiteral(boolean) => {
            let span = ctx.advance_span();
            Ok(SpannedExpr::new(Expr::Bool(boolean), span))
        }
        Token::Id(name_id) => {
            let span = ctx.advance_span();
            Ok(SpannedExpr::new(Expr::Var(name_id), span))
        }
        Token::Integer(name_id, notation) => {
            let span = ctx.advance_span();
            Ok(SpannedExpr::new(Expr::Integer(name_id, notation), span))
        }
        Token::Float(name_id, notation) => {
            let span = ctx.advance_span();

            Ok(SpannedExpr::new(Expr::Float(name_id, notation), span))
        }
        Token::Str(name_id) => {
            let span = ctx.advance_span();
            Ok(SpannedExpr::new(Expr::Str(name_id), span))
        }
        Token::Char(ch) => {
            let span = ctx.advance_span();

            Ok(SpannedExpr::new(Expr::Char(ch), span))
        }
        t if t.kind().is_terminator() => {
            ctx.advance_tok();

            let terminator = if t == Token::EOF { "<eof>" } else { "`@end`" };

            ctx.report_verbose(
                &format!("Expected expression, found {terminator}"),
                Branch::Type,
                interner,
            );

            Err(t)
        }
        t => {
            ctx.advance_tok();

            let fmtted_tok = parse_fmt::fmt_tok(t, interner);

            let msg = match t {
                Token::Invalid(_) => {
                    format!("Expected a valid expression, found invalid {fmtted_tok}")
                }
                Token::Keyword(_) => {
                    format!("Expected a valid expression, found keyword {fmtted_tok}",)
                }
                _ => format!("Expected a valid expression, found {fmtted_tok}"),
            };

            ctx.report_verbose(&msg, Branch::Expr, interner);
            return Err(Token::Poison);
        }
    }
}

fn parse_call_args(
    ctx: &mut ParserContext,
    budget: &ParserBudget,
    interner: &Intern,
) -> Result<Vec<SpannedExpr>, Token> {
    let mut args: Vec<SpannedExpr> = Vec::new();

    if ctx.peek_kind() == TokenKind::CParen {
        ctx.advance_tok();
        return Ok(args);
    }

    loop {
        let arg = parse_expr(ctx, 0, budget, interner)?;
        args.push(arg);

        if ctx.peek_tok() == Token::CParen {
            ctx.advance_tok();
            break;
        }

        if ctx.peek_tok() == Token::Comma && ctx.peek_ahead(1).tok == Token::CParen {
            ctx.advance_tok();
            ctx.advance_tok();
            break;
        }

        ctx.expect_verbose(
            TokenKind::Comma,
            "Expected ',' to separate arguments, found ",
            "",
            Branch::ArgList,
            interner,
        )?;
    }

    Ok(args)
}

fn parse_unary(
    ctx: &mut ParserContext,
    budget: &ParserBudget,
    interner: &Intern,
) -> Result<SpannedExpr, Token> {
    let _guard = budget.increase_depth().map_err(|_| {
        let msg = format!(
            "Reached max recursive depth of {}",
            budget.recursion_tracker.limit()
        );

        ctx.report_verbose(&msg, Branch::Type, interner);
        Token::Poison
    })?;
    match ctx.peek_tok() {
        //BUG: Unary does not properly apply self to member access
        Token::Hyphen => {
            let start = ctx.advance_span().start;
            let expr = parse_unary(ctx, budget, interner)?;

            let span = SourceSpan::new(ctx.region.region_id, start, expr.span.end);
            let unary = Unary::new(UnaryOp::Negate, Box::new(expr));

            Ok(SpannedExpr::new(Expr::Unary(unary), span))
        }
        Token::ExclamationPoint => {
            let start = ctx.advance_span().start;

            let expr = parse_unary(ctx, budget, interner)?;
            let span = SourceSpan::new(ctx.region.region_id, start, expr.span.end);

            let unary = Unary::new(UnaryOp::Not, Box::new(expr));

            Ok(SpannedExpr::new(Expr::Unary(unary), span))
        }
        Token::Tilde => {
            let start = ctx.advance_span().start;

            let expr = parse_unary(ctx, budget, interner)?;
            let span = SourceSpan::new(ctx.region.region_id, start, expr.span.end);

            let unary = Unary::new(UnaryOp::BitNot, Box::new(expr));

            Ok(SpannedExpr::new(Expr::Unary(unary), span))
        }
        _ => parse_primary(ctx, budget, interner),
    }
}

// ENFORCE TYPE NAMING FOR GENERICS AT LEAST
/// Recursive function for parsing all type expressions
fn parse_type_expr(
    ctx: &mut ParserContext,
    budget: &ParserBudget,
    interner: &Intern,
) -> Result<SpannedContainer<TypeExpr>, Token> {
    // SAFETY:
    //TODO: Foot
    let _guard = budget.increase_depth().map_err(|_| {
        let msg = format!(
            "Reached max recursive depth of {}",
            budget.recursion_tracker.limit()
        );

        ctx.report_verbose(&msg, Branch::Type, interner);
        Token::Poison
    })?;

    match ctx.peek_tok() {
        Token::Id(name_id) if ctx.peek_ahead(1).tok.kind() == TokenKind::OAngleBracket => {
            let start = ctx.advance_span().start;

            let args = parse_generic(ctx, budget, interner)?;
            let generic = Generic::new(name_id, args);

            let end = ctx.peek_behind(1).span.end;
            let span = SourceSpan::new(ctx.region.region_id, start, end);

            if ctx.peek_tok() == Token::StaticAccess {
                ctx.advance_tok();

                let mut ty_path =
                    vec![SpannedPathSegment::new(PathSegment::Generic(generic), span)];
                let mut rest = parse_static_path(ctx, budget, interner)?;

                ty_path.append(&mut rest);

                let path_end = ctx.peek_behind(1).span.end;
                let path_span = SourceSpan::new(ctx.region.region_id, start, path_end);

                Ok(SpannedContainer::new(TypeExpr::Path(ty_path), path_span))
            } else {
                let ty_expr = TypeExpr::Generic(generic);
                let spanned_ty_expr = SpannedContainer::new(ty_expr, span);

                Ok(spanned_ty_expr)
            }
        }
        Token::Id(_) if ctx.peek_ahead(1).tok.kind() == TokenKind::StaticAccess => {
            let start = ctx.peek_span().start;
            let ty_path = parse_static_path(ctx, budget, interner)?;
            let end = ctx.peek_behind(1).span.end;

            let span = SourceSpan::new(ctx.region.region_id, start, end);

            Ok(SpannedContainer::new(TypeExpr::Path(ty_path), span))
        }
        Token::Id(name_id) => {
            let span = ctx.advance_span();
            let ty_expr = TypeExpr::Var(name_id);

            Ok(SpannedContainer::new(ty_expr, span))
        }
        Token::Str(id) | Token::Integer(id, _) => {
            let tok = ctx.advance_tok();

            let fmt_tok = parse_fmt::fmt_tok(tok, interner);

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

            //TODO: Maybe it should return a boolean instead since, if the token is not known, it gets
            // `{}` around it anyways, so it's more so, the caller may or may not care about it
            // having an identifier, not that it can't format it
            let fmt_tok = parse_fmt::fmt_tok(t, interner);

            ctx.report_template("a type", &fmt_tok, Branch::Type, interner);
            //WARN:
            Err(Token::Poison)
        }
    }
}

/// Parses a `::` separated static access path into a list of `SpannedPathSegment`s.
/// Handles both plain identifiers and generic segments, and is shared between expression
/// and type-expression contexts.
fn parse_static_path(
    ctx: &mut ParserContext,
    budget: &ParserBudget,
    interner: &Intern,
) -> Result<Vec<SpannedPathSegment>, Token> {
    let mut static_path: Vec<SpannedPathSegment> = Vec::new();

    loop {
        let is_generic = ctx.peek_ahead(1).tok == Token::OAngleBracket;
        let is_static_access = ctx.peek_ahead(1).tok == Token::StaticAccess;

        if is_generic {
            let start = ctx.peek_span().start;
            let base_id = ctx.expect_id_verbose(
                TokenKind::Id,
                "Expected identifier after '::', found ",
                "",
                Branch::Type,
                interner,
            )?;

            let args = parse_generic(ctx, budget, interner)?;
            let end = ctx.peek_behind(1).span.end;

            let generic = Generic::new(base_id, args);

            let span = SourceSpan::new(ctx.region.region_id, start, end);
            let segment = SpannedPathSegment::new(PathSegment::Generic(generic), span);

            static_path.push(segment);

            if ctx.peek_tok() == Token::StaticAccess {
                ctx.advance_tok();
            } else {
                break;
            }
        // This is just the normal case of an identifier after a dot
        } else if is_static_access {
            let span = ctx.peek_span();
            let name_id = ctx.expect_id_verbose(
                TokenKind::Id,
                "Expected identifier after '::', found ",
                "",
                Branch::Type,
                interner,
            )?;

            ctx.advance_tok();

            let segment = SpannedPathSegment::new(PathSegment::Ident(name_id), span);
            static_path.push(segment);
        }

        // If there's no dot then that means that the current token must be an identifier that is
        // the field to access
        if !is_static_access {
            let span = ctx.peek_span();
            let final_ident = ctx.expect_id_verbose(
                TokenKind::Id,
                "Expected complete member access, found ",
                "",
                Branch::Type,
                interner,
            )?;

            let segment = SpannedPathSegment::new(PathSegment::Ident(final_ident), span);
            static_path.push(segment);
            break;
        }
    }

    Ok(static_path)
}

/// Parses assuming that within "List<i32>" the "List" part was skipped, which would leave <i32>
/// to be handled
fn parse_generic(
    ctx: &mut ParserContext,
    budget: &ParserBudget,
    interner: &Intern,
) -> Result<Vec<SpannedContainer<TypeExpr>>, Token> {
    let _guard = budget.increase_depth();
    ctx.expect_verbose(
        TokenKind::OAngleBracket,
        "Expected '<' to declare generic, found ",
        "",
        Branch::Type,
        interner,
    )?;

    let mut inputs: Vec<SpannedContainer<TypeExpr>> = Vec::new();

    let input = parse_type_expr(ctx, budget, interner)?;
    inputs.push(input);

    // Doesn't need terminator check since the loop would need to be continued on purpose through
    // user-intent for this to not just error
    while ctx.peek_kind() == TokenKind::Comma {
        ctx.advance_tok();

        let other_input = parse_type_expr(ctx, budget, interner)?;
        inputs.push(other_input);
    }

    ctx.expect_verbose(
        TokenKind::CAngleBracket,
        "Expected '>' to close generic parameters, found ",
        "",
        Branch::Type,
        interner,
    )?;

    Ok(inputs)
}

fn handle_struct_fields(
    ctx: &mut ParserContext,
    struct_name: &str,
    budget: &ParserBudget,
    interner: &Intern,
) -> Result<Vec<AbstractTypeDef>, Token> {
    let mut fields: Vec<AbstractTypeDef> = Vec::new();
    // Skips since if it failed that probably means it's not a CCurly after
    //
    // Example: "struct Point { x: i32, y: i32, z: }"
    // It failed at z, but a colon is after z. This could also be accounted for by the "recovery"
    // internally but it doesn't really make a difference here being safer.
    let mut check_end = true;

    while !ctx.peek_tok().kind().is_terminator() && ctx.peek_tok() != Token::CCurlyBracket {
        let ty = match parse_typedef(ctx, budget, interner) {
            Ok(t) => t,
            Err(_) => {
                check_end = false;
                break;
            }
        };

        fields.push(ty);

        if ctx.peek_tok() == Token::CCurlyBracket {
            break;
        }
    }

    if check_end {
        ctx.expect_verbose(
            TokenKind::CCurlyBracket,
            &format!("Expected field or '}}' to close struct `{struct_name}`, found "),
            "",
            Branch::Section(NestBranch::StructType.into()),
            interner,
        )?;
    }

    Ok(fields)
}

/// Assumes the '{' was advanced
fn handle_enum_variants(
    ctx: &mut ParserContext,
    enum_name: &str,
    budget: &ParserBudget,
    interner: &Intern,
) -> Result<Vec<AbstractVariant>, Token> {
    let mut variants: Vec<AbstractVariant> = Vec::new();
    let mut check_end = true;

    //NOTE: ALSO SUSPICIOUS
    while !ctx.peek_tok().kind().is_terminator() && ctx.peek_tok() != Token::CCurlyBracket {
        let variant = match parse_variant(ctx, budget, interner) {
            Ok(v) => v,
            Err(_) => {
                check_end = true;
                break;
            }
        };
        variants.push(variant);

        if ctx.peek_tok() == Token::CCurlyBracket {
            break;
        }

        if ctx.peek_kind() == TokenKind::Comma {
            ctx.advance_tok();
        }
    }

    if check_end {
        ctx.expect_verbose(
            TokenKind::CCurlyBracket,
            &format!("Expected variant or '}}' to close enum `{enum_name}`, found "),
            "",
            Branch::Section(NestBranch::EnumType.into()),
            interner,
        )?;
    }

    Ok(variants)
}

/// Variant-specific parser that account for if there is a type declared with the variant or not
fn parse_variant(
    ctx: &mut ParserContext,
    budget: &ParserBudget,
    interner: &Intern,
) -> Result<AbstractVariant, Token> {
    let name_span = ctx.peek_span();

    let name_id = ctx.expect_id_verbose(
        TokenKind::Id,
        "Expected identifier for variant, found ",
        "",
        Branch::Section(NestBranch::EnumType.into()),
        interner,
    )?;

    let ty_opt: Option<SpannedContainer<TypeExpr>> = if ctx.peek_kind() == TokenKind::Colon {
        ctx.advance_tok();
        let ty = parse_type_expr(ctx, budget, interner)?;
        Some(ty)
    } else {
        None
    };

    let conds = if ctx.peek_kind() == TokenKind::OBracket {
        handle_conds(ctx, budget, interner).unwrap_or_default()
    } else {
        Vec::new()
    };

    let args = if ctx.peek_kind() == TokenKind::HashSymbol {
        handle_directives(ctx, interner).unwrap_or_default()
    } else {
        Vec::new()
    };

    let variant = AbstractVariant::new(name_id, name_span, ty_opt, conds, args);

    Ok(variant)
}

// Egregious naming scheme
fn handle_directives(
    ctx: &mut ParserContext,
    interner: &Intern,
) -> Result<Vec<AbstractDirective>, Token> {
    let mut args: Vec<AbstractDirective> = Vec::new();

    // Doesn't need terminator check since the loop would need to be continued on purpose through
    // user-intent for this to not just error
    while ctx.peek_kind() == TokenKind::HashSymbol {
        ctx.advance_tok();
        args.push(parse_directive(ctx, interner)?);
    }

    Ok(args)
}

fn parse_directive(ctx: &mut ParserContext, interner: &Intern) -> Result<AbstractDirective, Token> {
    let name_span = ctx.peek_span();
    let name_id = ctx.expect_id_verbose(
        TokenKind::Id,
        "Unknown directive ",
        "",
        Branch::Directive,
        interner,
    )?;

    let sp_name_id = SpannedContainer::new(name_id, name_span);
    let abs_directive = AbstractDirective::new(sp_name_id);

    Ok(abs_directive)
}

// Alias is this only one that uses this so_+@$_$@
fn parse_alias_decl(
    ctx: &mut ParserContext,
    budget: &ParserBudget,
    interner: &Intern,
) -> Result<Vec<AbstractParam>, Token> {
    let mut params: Vec<AbstractParam> = Vec::new();

    // I guess this could get a loop count at least?
    //
    // Doesn't need terminator check since the loop would need to be continued on purpose through
    // user-intent for this to not just error
    while ctx.peek_kind() != TokenKind::CParen {
        let param = match ctx.peek_tok() {
            Token::Id(name_id) => {
                let span = ctx.advance_span();

                ctx.expect_verbose(
                    TokenKind::Colon,
                    "Expected ':' to define a type or boundary, found ",
                    "",
                    Branch::Neutral(NeutralBranch::Alias),
                    interner,
                )?;

                let ty_expr = parse_type_expr(ctx, budget, interner)?;

                AbstractParam::new(name_id, span, ty_expr)
            }
            Token::EOF => return Err(Token::Poison),
            _ => {
                ctx.advance_tok();

                let msg = "Only identifiers can be within alias parameters";
                ctx.report_verbose(&msg, Branch::Cond, interner);
                return Err(Token::Poison);
            }
        };

        params.push(param);

        if ctx.peek_kind() == TokenKind::CParen {
            break;
        }

        _ = ctx.expect_verbose(
            TokenKind::Comma,
            "Expected ',' to separate arguments, found ",
            "",
            Branch::ArgList,
            interner,
        )?;
    }

    ctx.expect_verbose(
        TokenKind::CParen,
        "Expected ')' to close declaration, found ",
        "",
        Branch::ArgList,
        interner,
    )?;

    Ok(params)
}

fn handle_conds(
    ctx: &mut ParserContext,
    budget: &ParserBudget,
    interner: &Intern,
) -> Result<Vec<SpannedExpr>, Token> {
    let mut conds: Vec<SpannedExpr> = Vec::new();
    // This count cannot end the definition since it would prevent arguments from being viewed
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

    // Doesn't need terminator check since the loop would need to be continued on purpose through
    // user-intent for this to not just error (I think)
    while ctx.peek_tok() != Token::CBracket {
        let cond = parse_expr(ctx, 0, budget, interner)?;
        conds.push(cond);

        if ctx.peek_tok() == Token::CBracket {
            break;
        }

        ctx.expect_verbose(
            TokenKind::Comma,
            "Expected ',' to separate arguments or ']' to close, found ",
            "",
            Branch::Cond,
            interner,
        )?;
    }

    _ = ctx.expect_verbose(
        TokenKind::CBracket,
        "Expected ']' at end of condition, found ",
        "",
        Branch::Cond,
        interner,
    );

    Ok(conds)
}

//NOTE: Could make the first pass only resolve the basics so that module resolution for local
//imports are handled without giving the module passer too much syntax knowledge, but fine for now.
/// Parses the only language-known prefix `export` and returns an error in the case of an issue like
/// duplicate exports, otherwise will return `is_priv` on `Ok`
fn parse_export(ctx: &mut ParserContext, interner: &Intern) -> Result<bool, ()> {
    let mut is_priv = true;

    while let Token::Keyword(kw) = ctx.peek_tok()
        && kw == Keyword::Export
    {
        if !is_priv {
            ctx.advance_tok();

            ctx.report_verbose(
                "Cannot apply `export` more than once",
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

/// Helper for solely reporting export errors
fn report_export(ctx: &mut ParserContext, fmtted: Formatted, branch: Branch, interner: &Intern) {
    ctx.report_verbose(
        &format!("Cannot use `export` on `{}`", fmtted),
        branch,
        interner,
    );
}
