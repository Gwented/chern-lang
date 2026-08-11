// Static access help messages
use chrn_utils::{
    chrn_config::ChrnConfig,
    err_codes::ErrorCode,
    id_types::InternedId,
    intern::Intern,
    s_suffix,
    source_map::{
        source_diagnostic::{
            DiagnosticLevel, SourceDiagnostic, SourceDiagnosticBuilder, SourceDiagnosticSink,
            SourceDiagnosticSummary, annotations::AnnotationKind,
        },
        source_region::SourceRegion,
        source_span::SourceSpan,
    },
};
use lang::{
    algo::{self, FuzzyMatch},
    fmter::Formattable,
    keywords::Keyword,
};

use crate::{
    lexer::token::{self, Notation, SpannedToken, Token, TokenKind},
    parser::{
        Evidence, InitialEvidence, NeutralBranch, SectionBranch, SemanticSituation, branch::Branch,
        parse_fmt,
    },
};

use super::NestBranch;

// C_ == current. A_ == ahead

// ALL SET LOGIC AND PARSE LOGIC NEED TO WORK WITH EACH OTHER
//TODO: Most optimal solution for this is to act off of only section context, so not as granular

//NOTE: The basic exit sets should ONLY have tokens that will ALWAYS be stopped on.
const C_BASE_EXIT_SET: u64 = token::EOF | token::INVALID | token::KEYWORD;
const A_BASE_EXIT_SET: u64 = token::SLIM_ARROW;

const C_STMT_NEUTRAL_SET: u64 = C_BASE_EXIT_SET /*| token::Keyword*/ ;

const C_BRANCH_VAR_SET: u64 = C_BASE_EXIT_SET;
const A_BRANCH_VAR_SET: u64 = A_BASE_EXIT_SET | token::COLON;

// WARN: NestType should probably be responsible for C_CURLY but maybe not
const C_BRANCH_TYPE_SET: u64 = C_BASE_EXIT_SET;

const A_BRANCH_TYPE_SET: u64 = A_BASE_EXIT_SET | token::COLON;

// Probably shouldn't account for hash symbol since it is not apart of the loop
const C_BRANCH_COND_SET: u64 = C_BASE_EXIT_SET | token::C_CURLY_BRACKET;
const A_BRANCH_COND_SET: u64 = A_BASE_EXIT_SET | token::COLON;

const C_BRANCH_TYPE_ARGS_SET: u64 = C_BASE_EXIT_SET | token::HASH_SYMBOL;
const A_BRANCH_TYPE_ARGS_SET: u64 = A_BASE_EXIT_SET | token::COLON;

//TODO: Find out what tuning works best for these if they are going to stay.
const C_BRANCH_FUNC_SET: u64 = C_BASE_EXIT_SET | token::C_PAREN;
const A_BRANCH_FUNC_SET: u64 = A_BASE_EXIT_SET | token::C_BRACKET;

/// Parser context struct that orchestrates parser as well as reports errors
// p_ctx
#[derive(Debug)]
pub(super) struct ParserContext<'a> {
    cfg: &'a ChrnConfig,
    pub(super) region: &'a SourceRegion,
    toks: &'a [SpannedToken],
    pos: usize,
    //TODO: Maybe a budget too
    pub(super) summary: SourceDiagnosticSummary,
}

impl<'a> ParserContext<'a> {
    pub(super) fn new(
        cfg: &'a ChrnConfig,
        region: &'a SourceRegion,
        toks: &'a [SpannedToken],
    ) -> ParserContext<'a> {
        ParserContext {
            cfg,
            region,
            toks,
            pos: 0,
            summary: SourceDiagnosticSummary::default(),
        }
    }

    /// Returns an interned name id on success and the failed token on error.
    /// Format: {bmsg}{found}{amsg}
    pub(super) fn expect_id_verbose(
        &mut self,
        expected: TokenKind,
        bmsg: &str,
        amsg: &str,
        initial_evidence: InitialEvidence,
        interner: &Intern,
    ) -> Result<InternedId, Token> {
        let found = self.advance();
        let branch = initial_evidence.branch;

        let fmtted_tok = match found.tok {
            Token::Id(id) | Token::Str(id) | Token::Integer(id, _) | Token::Float(id, _) => {
                if found.tok.kind() == expected {
                    return Ok(id);
                } else {
                    parse_fmt::fmt_tok(found.tok, interner)
                }
            }
            t => parse_fmt::fmt_tok(t, interner),
        };

        let core_msg = format!("{bmsg}{fmtted_tok}{amsg}");

        let (mut builder, situation_opt) = self.create_diag_builder(&found, core_msg);
        // If EOF then override otherwise keep same semantics
        let situation = situation_opt.unwrap_or(initial_evidence.situation);

        let evidence = Evidence::new(
            initial_evidence.env,
            situation,
            expected,
            found.clone(),
            initial_evidence.branch,
        );

        builder = self.try_assistance(builder, &evidence, interner);

        self.summary.push_diag(builder.build());

        self.recover(branch);

        Err(found.tok)
    }

    /// Creates basic diagnostic with `AnnotationKind::Primary` using the span of the found token.
    /// If a occurrence such as EOF is detected as the error, the `SemanticSituation` will be
    /// returned so that assistance can enhance the error message where needed. It can be ignored.
    pub(super) fn create_diag_builder(
        &mut self,
        found: &SpannedToken,
        core_msg: String,
    ) -> (SourceDiagnosticBuilder, Option<SemanticSituation>) {
        let builder =
            SourceDiagnostic::builder(None, DiagnosticLevel::Error, core_msg, self.region.path_id)
                .add_annotation(found.span, AnnotationKind::Primary, None);
        let situation_opt = if found.tok.kind().is_terminator() {
            Some(SemanticSituation::ReachedEOF)
        } else {
            None
        };

        (builder, situation_opt)
    }

    /// Returns an interned name id on success and the failed token on error.
    /// Format: {bmsg}{found}{amsg}
    pub(super) fn expect_kw_verbose(
        &mut self,
        bmsg: &str,
        amsg: &str,
        initial_evidence: InitialEvidence,
        interner: &Intern,
    ) -> Result<Keyword, Token> {
        let found = self.advance();
        let branch = initial_evidence.branch;

        if let Token::Keyword(kw) = found.tok {
            return Ok(kw);
        }

        let fmtted_tok = parse_fmt::fmt_tok(found.tok, interner);

        // Well maybe fmt_tok should be a method at this point, or at least an associated function
        let core_msg = format!("{bmsg}{fmtted_tok}{amsg}");

        let (mut builder, situation_opt) = self.create_diag_builder(&found, core_msg);
        // If EOF then override otherwise keep same semantics
        let situation = situation_opt.unwrap_or(initial_evidence.situation);

        let evidence = Evidence::new(
            initial_evidence.env,
            situation,
            TokenKind::Keyword,
            found.clone(),
            initial_evidence.branch,
        );
        builder = self.try_assistance(builder, &evidence, interner);

        self.summary.push_diag(builder.build());

        self.recover(branch);

        Err(found.tok)
    }

    // BOF
    /// Intended for basic errors that need little context after
    /// This must ALWAYS be advanced before usage due to the found token always being assumed to be
    /// the previous token.
    pub(super) fn report_verbose<S: Into<String>>(
        &mut self,
        core_msg: S,
        initial_evidence: InitialEvidence,
        interner: &Intern,
    ) {
        let found = self.peek_behind(1);
        let branch = initial_evidence.branch;

        let (mut builder, situation_opt) = self.create_diag_builder(&found, core_msg.into());
        // If EOF then override otherwise keep same semantics
        let situation = situation_opt.unwrap_or(initial_evidence.situation);

        let evidence = Evidence::new(
            initial_evidence.env,
            situation,
            TokenKind::Poison,
            found.clone(),
            initial_evidence.branch,
        );
        builder = self.try_assistance(builder, &evidence, interner);

        self.summary.push_diag(builder.build());

        self.recover(branch);
    }

    /// Returns the found token on success and failure.
    /// Format: {bmsg}{found}{amsg}
    // TODO:  Maybe lazily evaluate since searching the interner by default is a weird performance
    // hit. Probably.
    pub(super) fn expect_verbose(
        &mut self,
        expected: TokenKind,
        bmsg: &str,
        amsg: &str,
        initial_evidence: InitialEvidence,
        interner: &Intern,
    ) -> Result<Token, Token> {
        let found = self.advance();
        let branch = initial_evidence.branch;

        if found.tok.kind() != expected {
            let fmtted_tok = parse_fmt::fmt_tok(found.tok, interner);

            let core_msg = format!("{bmsg}{fmtted_tok}{amsg}");

            let (mut builder, situation_opt) = self.create_diag_builder(&found, core_msg);
            // If EOF then override otherwise keep same semantics
            let situation = situation_opt.unwrap_or(initial_evidence.situation);

            let evidence = Evidence::new(
                initial_evidence.env,
                situation,
                expected,
                found.clone(),
                initial_evidence.branch,
            );
            builder = self.try_assistance(builder, &evidence, interner);

            self.summary.push_diag(builder.build());
            self.recover(branch);

            return Err(found.tok);
        }

        Ok(found.tok)
    }

    /// More composable "Expected but found" error.
    /// This must ALWAYS be advanced before usage due to the found token always being assumed to be
    /// the previous token.
    /// Format: "Expected {emsg}, found {fmsg}"
    pub(super) fn report_template(
        &mut self,
        emsg: &str,
        fmsg: &str,
        initial_evidence: InitialEvidence,
        interner: &Intern,
    ) {
        let found = self.peek_behind(1);
        let branch = initial_evidence.branch;

        let core_msg = format!("Expected {emsg}, found {fmsg}");

        let (mut builder, situation_opt) = self.create_diag_builder(&found, core_msg);
        // If EOF then override otherwise keep same semantics
        let situation = situation_opt.unwrap_or(initial_evidence.situation);

        let evidence = Evidence::new(
            initial_evidence.env,
            situation,
            TokenKind::Poison,
            found.clone(),
            initial_evidence.branch,
        );
        builder = self.try_assistance(builder, &evidence, interner);
        self.summary.push_diag(builder.build());

        self.recover(branch);
    }

    /// Where error recovery semantics are handled, given a set associated with a particulra branch.
    fn recover(&mut self, branch: Branch) {
        let (current_targets, next_targets) = self.get_set_from_branch(branch);

        if !self.peek_kind().is_terminator() {
            while self.pos < self.toks.len() + 2
                && (self.peek_kind().to_u64() & current_targets) == 0
                && (self.peek_ahead(1).tok.kind().to_u64() & next_targets) == 0
            {
                self.advance();
            }
        }
    }

    // ()
    fn get_set_from_branch(&self, branch: Branch) -> (u64, u64) {
        match branch {
            Branch::Broken => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
            Branch::Searching => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
            Branch::Neutral(neutral_branch) => match neutral_branch {
                NeutralBranch::Searching => (C_STMT_NEUTRAL_SET, A_BASE_EXIT_SET),
                NeutralBranch::Bind => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
                NeutralBranch::Alias => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
                NeutralBranch::Let => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
                NeutralBranch::Import => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
            },
            Branch::Section(sect_branch) => match sect_branch {
                SectionBranch::Searching => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
                SectionBranch::Var => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
                SectionBranch::Nest(_) => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
                SectionBranch::Complex => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
                SectionBranch::Override => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
            },
            Branch::Expr => (C_BRANCH_COND_SET, A_BRANCH_COND_SET),
            Branch::Cond => (C_BRANCH_COND_SET, A_BRANCH_COND_SET),
            Branch::Type => (C_BRANCH_TYPE_SET, A_BRANCH_TYPE_SET),
            Branch::ArgList => (C_BRANCH_FUNC_SET, A_BRANCH_FUNC_SET),
            Branch::Directive => (C_BRANCH_TYPE_ARGS_SET, A_BRANCH_TYPE_ARGS_SET),
        }
    }

    /// May or may not mutate the given builder if any improvement is found for error message's
    /// quality. Goes through a mostly semantic check first, then a more literal tree of the stream
    /// of tokens gone by so that the highest quality message is tried for before returning.
    fn try_assistance(
        &self,
        // Msg to pick if nothing is matched as an error msg
        builder: SourceDiagnosticBuilder,
        evidence: &Evidence,
        interner: &Intern,
    ) -> SourceDiagnosticBuilder {
        let (builder, changed) = self.try_semantic_assistance(builder, evidence, interner);

        if changed {
            return builder;
        }

        self.try_tree_assistance(
            builder,
            evidence.expected,
            &evidence.found,
            evidence.branch,
            interner,
        )
    }

    /// Takes builder, and returns it with a boolean representing if it was altered or not.
    /// If `true` then it was altered, `false` means nothing was changed
    ///
    /// The intention of semantic assistance is to infer intent in how to alter the builder based
    /// off of mostly semantic, and some tree-related information, rather than just tree information.
    /// This is to lower instances like, name binding when a keyword was used, which isn't bespoke
    /// in any form but the tree is so explicit that this can't be done without semantic
    /// understanding.
    fn try_semantic_assistance(
        &self,
        mut builder: SourceDiagnosticBuilder,
        evidence: &Evidence,
        interner: &Intern,
    ) -> (SourceDiagnosticBuilder, bool) {
        // True by default since false would be more non-trivial to change
        let mut changed = true;

        // Typed?
        match &evidence.situation {
            SemanticSituation::IdentBinding => match &evidence.found.tok {
                Token::Keyword(_) | Token::BoolLiteral(_) | Token::Integer(_, _) => {
                    let s = match evidence.found.tok {
                        Token::Keyword(kw) => kw.to_fmt().to_string(),
                        Token::BoolLiteral(boolean) => boolean.to_string(),
                        // Notation doesn't matter
                        Token::Integer(id, _) => interner.search(id).into(),
                        //WARN: IGNORE THIS. DO NOT COMMENT ON THIS.
                        _ => unsafe {
                            std::hint::unreachable_unchecked();
                        },
                    };
                    builder = builder.add_help(format!("Can be escaped with `e#{}`", s));
                }
                _ => changed = false,
            },
            SemanticSituation::UnexpectedToken => changed = false,
            SemanticSituation::TypeBinding => changed = false,

            SemanticSituation::ValueBinding => changed = false,
            SemanticSituation::DirectiveParsing => match evidence.found.tok {
                Token::Id(id) => {
                    // NOTE: We can't actually make it here. At least not reliably
                    let bytes = interner.search(id).as_bytes();
                    let similar = algo::fuzzy_match(bytes, FuzzyMatch::Directive);

                    if similar.len() > 0 {
                        let s_suffix = s_suffix!(similar.len());
                        let similar_msg = Self::fmt_founds(
                            &similar,
                            &format!("Similar directive{s_suffix}:"),
                            "`",
                        );
                        builder = builder.add_help(similar_msg);
                    } else {
                        changed = false;
                    }
                }
                _ => changed = false,
            },
            SemanticSituation::ArgList => changed = false,
            SemanticSituation::UnclosedDelimiter => changed = false,
            SemanticSituation::MissingStartDelimiter => changed = false,
            SemanticSituation::ReachedEOF => {
                let terminator_str: &str = match evidence.found.tok {
                    Token::EOF => "<eof>",
                    Token::End => "`@end`",
                    // Ok but shouldn't this be unreachable?
                    // Um
                    _ => "",
                };

                // For a `ReachedEOF` error that must first be a token that didn't expect eof,
                // meaning this is impossible to fail since it requires a token before it.
                let prev_span = self.toks[self.toks.len() - 2].span;

                builder = SourceDiagnostic::builder(
                    ErrorCode::ConfigLoadErr.into(),
                    DiagnosticLevel::Error,
                    //Yeah, no? No?
                    builder.build().core_msg,
                    self.region.path_id,
                )
                .add_annotation(
                    prev_span,
                    AnnotationKind::Secondary,
                    format!("Token before {terminator_str}").into(),
                )
                .add_annotation(
                    evidence.found.span,
                    AnnotationKind::Primary,
                    format!("Unexpected {terminator_str}").into(),
                );
            }
            SemanticSituation::KeywordBinding => match evidence.found.tok {
                Token::Id(id) => {
                    let bytes = interner.search(id).as_bytes();
                    let similar = algo::fuzzy_match(bytes, FuzzyMatch::KW);

                    if similar.len() > 0 {
                        let s_suffix = s_suffix!(similar.len());
                        //TODO: Scope reasoning to not recommend specific KW if invalid in the
                        //particular scope
                        let similar_msg =
                            Self::fmt_founds(&similar, &format!("Similar keyword{s_suffix}:"), "`");
                        builder = builder.add_help(similar_msg);
                    } else {
                        changed = false;
                    }
                }
                _ => changed = false,
            },
        };

        (builder, changed)
    }

    ////????????
    /// Checks available known branching to where a help message can be pushed
    fn try_tree_assistance(
        &self,
        mut builder: SourceDiagnosticBuilder,
        expected: TokenKind,
        found: &SpannedToken,
        branch: Branch,
        interner: &Intern,
    ) -> SourceDiagnosticBuilder {
        let prev_prev_tok = match self.toks.get(self.pos.saturating_sub(3)).clone() {
            Some(t) => t,
            None => return builder,
        };

        let prev_tok = match self.toks.get(self.pos.saturating_sub(2)).clone() {
            Some(t) => t,
            None => return builder,
        };

        let prev_kind = prev_tok.tok.kind();

        let next_kind = self
            .toks
            .get(self.pos + 1)
            .map(|t| t.tok.kind())
            .unwrap_or(TokenKind::Poison);

        let next_next_kind = self
            .toks
            .get(self.pos + 2)
            .map(|t| t.tok.kind())
            .unwrap_or(TokenKind::Poison);

        let builder = match branch {
            Branch::Neutral(neutral_branch) => match neutral_branch {
                NeutralBranch::Let => match found.tok {
                    Token::Keyword(kw) if expected == TokenKind::Id => {
                        let help = format!("Can be escaped with `e#{}`", kw.to_fmt());
                        builder.add_help(help)
                    }
                    Token::Colon
                        // : [Token] =
                        if expected == TokenKind::Assign && next_kind == TokenKind::Assign =>
                    {
                        builder.add_note("`let` variables are only inferred".to_string())
                    }
                    _ => builder,
                },
                NeutralBranch::Searching => match found.tok {
                    // Found stray unrecognizable identifier in neutral
                    Token::Id(name_id) | Token::Invalid(name_id) => {
                        let found_bytes = interner.search(name_id).as_bytes();

                        // Statements and sections are possible so both are tried
                        let similar_opt =
                            algo::fuzzy_match_with_fmtted(found_bytes, FuzzyMatch::Stmt)
                                .is_none()
                                .then_some(algo::fuzzy_match_with_fmtted(
                                    found_bytes,
                                    algo::FuzzyMatch::Sect,
                                ));

                        // Uh huh
                        // Ok
                        if let Some(Some((similar_vec, fmtted_ty))) = similar_opt {
                            let help = Self::fmt_founds(
                                &similar_vec,
                                &format!("Found similar {fmtted_ty}"),
                                "`",
                            );

                            builder.add_help(help)
                        } else {
                            builder
                        }
                    }
                    _ => builder,
                },
                // TODO: Help for, alias default(x <- Send help for type annotations) = []
                NeutralBranch::Alias => match found.tok {
                    // Alias missing parameters
                    Token::Assign
                        if prev_kind == TokenKind::Id && next_kind != TokenKind::OParen =>
                    {
                        // This reward hack has to go
                        // let Token::Id(id) = prev_tok.tok else {
                        //     return None;
                        // };
                        //
                        // let name = interner.search(id as usize);

                        // let help_diag = reporter::help_transform(
                        //     name,
                        //     &format!("{name}()"),
                        //     self.settings.can_color,
                        // );
                        //
                        // // It looks weird now
                        // let help = reporter::standardize_help(&help_diag, self.settings.can_color);

                        // Some(help)
                        builder
                    }
                    _ => builder,
                },
                _ => builder,
            },
            Branch::Section(sect_branch) => match sect_branch {
                SectionBranch::Var => match found.tok {
                    // ident: core.i32
                    Token::Dot
                        if expected == TokenKind::Id
                            && prev_prev_tok.tok.kind() == TokenKind::Colon =>
                    {
                        let help = "Was this meant to be `::`?".to_string();
                        builder.add_help(help)
                    }
                    Token::Keyword(kw)
                        if expected == TokenKind::Id && next_kind == TokenKind::Colon =>
                    {
                        let help = format!("Can be escaped with `e#{}`", kw.to_fmt());
                        builder.add_help(help)
                    }
                    Token::Str(id)
                        if expected == TokenKind::Colon && prev_kind == TokenKind::Id =>
                    {
                        let Token::Id(possible_kw_id) = prev_tok.tok else {
                            return builder;
                        };

                        if let Some(kw) = Keyword::try_from_interned_id(possible_kw_id) {
                            let help = format!(
                                "If this was meant to use the statement `{}`, place this within `neutral`, which is the area before any section is used",
                                kw.to_fmt()
                            );

                            return builder.add_note(help);
                        }

                        builder
                    }
                    Token::OParen if expected == TokenKind::Colon => {
                        // if let Token::Id(prev_id) = prev_tok.tok {
                        //     panic!("CApi");
                        // }
                        //
                        builder
                        // let msg = "Is this missing '[' to define conditions?";
                        //
                        // let span = SourceSpan::new(prev_tok.span.start, found.span.end);
                        //
                        // let fmt_help = reporter::form_suggest_diag(
                        //     &self.metadata.src_bytes,
                        //     &span,
                        //     "+",
                        //     "[",
                        //     true,
                        //     self.metadata.can_color,
                        // );
                        //
                        // let msg = reporter::standardize_help(&msg, self.metadata.can_color);
                        //
                        // let built_help = format!("{fmt_help}\n{msg}");
                        //
                        // Some(built_help)
                    }
                    _ => builder,
                },
                SectionBranch::Nest(branch) => match branch {
                    NestBranch::NestStart => match found.tok {
                        // If in `nest->` and found, struct|enum [keyword]
                        Token::Keyword(kw) if expected == TokenKind::Id => {
                            if let Token::Keyword(kw) = prev_tok.tok {
                                if kw == Keyword::Struct || kw == Keyword::Enum {
                                    let help = format!("Can be escaped with `e#{}`", kw.to_fmt());

                                    return builder.add_help(help);
                                }
                            }

                            builder
                        }
                        _ => builder,
                    },
                    NestBranch::EnumType => builder,
                    NestBranch::StructType => builder,
                    NestBranch::Expr => builder,
                },
                // SectionBranch::NestType => todo!(),
                // SectionBranch::NestEnum => todo!(),
                SectionBranch::Complex => match found.tok {
                    Token::StaticAccess if expected == TokenKind::OCurlyBracket => {
                        //NOTE: Not sure if this should stick
                        // Also very normal sized message.
                        let note =
                            "Static access is not permitted within config declarations.\n  If this was a module namespace, declare this in it's module of origin.\n  If this was a type namespace, this must be defined inside the config itself using available syntax."
                                .to_string();
                        builder.add_note(note)
                    }
                    // Catching if the scenario ".option1 = 3 .option = 5" was reached which results
                    // in a member access related error.
                    //
                    // Change to colons?
                    Token::Assign
                        if expected == TokenKind::CCurlyBracket
                            && prev_tok.tok.kind() == TokenKind::Id
                            && prev_prev_tok.tok == Token::Dot =>
                    {
                        let help = "May be missing a comma to separate previous option".to_string();
                        builder.add_help(help)
                    }
                    _ => builder,
                },
                // SectionBranch::Override => todo!(),
                _ => match found.tok {
                    Token::Id(ident_id) | Token::Invalid(ident_id) => {
                        let found_bytes = interner.search(ident_id).as_bytes();

                        // Maybe this should return None if it directly IS a direct match since it is
                        // just a range check
                        let similar_vec = algo::fuzzy_match(found_bytes, algo::FuzzyMatch::Sect);

                        if !similar_vec.is_empty() {
                            let help = Self::fmt_founds(
                                &similar_vec,
                                &format!("Found similar section"),
                                "`",
                            );

                            builder = builder.add_help(help);
                        }

                        builder
                    }
                    _ => builder,
                },
            },
            // Branch::Expr => todo!(),
            Branch::Cond => match found.tok {
                Token::Id(id) if expected == TokenKind::CBracket => {
                    let help = "Is there a missing comma to separate conditions?".to_string();
                    builder.add_help(help)
                }
                Token::CBracket if prev_kind == TokenKind::Comma => {
                    let help = "Remove trailing ',' or add a condition".to_string();
                    builder.add_help(help)
                }
                _ => builder,
            },
            Branch::Type => match found.tok {
                Token::Keyword(kw) => {
                    let help = format!("Can be escaped with `e#{}`", kw.to_fmt());
                    builder.add_help(help)
                }
                Token::Dot => {
                    let help = "Was this meant to be `::`?".to_string();
                    builder.add_help(help)
                }
                Token::CAngleBracket if prev_kind == TokenKind::Comma => {
                    let help = "Was there a trailing ',' ?".to_string();
                    builder.add_help(help)
                }
                _ => builder,
            },
            // Branch::FuncArgs => todo!(),
            Branch::Directive => match found.tok {
                Token::Id(name_id) => {
                    let found_bytes = interner.search(name_id).as_bytes();
                    let similar_vec = algo::fuzzy_match(found_bytes, algo::FuzzyMatch::Directive);

                    if !similar_vec.is_empty() {
                        let help = Self::fmt_founds(
                            &similar_vec,
                            &format!("Found similar directive"),
                            "`",
                        );

                        builder = builder.add_help(help);
                    }

                    builder
                }
                _ => builder,
            },
            _ => builder,
        };

        builder.into()
    }

    // ??????
    fn fmt_founds(found: &[&str], header: &str, quote: &str) -> String {
        let mut out = format!("{header} ");
        for (i, similar) in found.iter().enumerate() {
            out.push_str(&format!("{quote}{similar}{quote}"));
            if i + 1 != found.len() {
                out.push_str(", ");
            }
        }
        out
    }

    /// Intended to handle the case where EOF is reached due to errors likely wanting to show the
    /// last token TO EOF, rather than just EOF
    fn safely_handle_span(&self, found: &SpannedToken) -> Vec<SourceSpan> {
        if found.tok.kind().is_terminator() {
            // Minus 2 since we advanced at the beginning
            let start_span = self.toks.get(self.pos - 2).unwrap_or(found).span.clone();
            vec![start_span, found.span.clone()]
        } else {
            vec![found.span.clone()]
        }
    }

    pub(super) fn peek(&mut self) -> SpannedToken {
        self.toks[self.pos].clone()
    }

    pub(super) fn peek_tok(&mut self) -> Token {
        self.toks.get(self.pos).map(|t| t.tok).unwrap_or(Token::EOF)
    }

    pub(super) fn peek_kind(&self) -> TokenKind {
        self.toks
            .get(self.pos)
            .map(|t| t.tok.kind())
            .unwrap_or(TokenKind::EOF)
    }

    // Not sure about this default
    // Probably will become option for these
    pub(super) fn peek_ahead(&self, dest: usize) -> SpannedToken {
        self.toks
            .get(self.pos + dest)
            .map(|st| st.clone())
            // But what if the final token isn't EOF..
            .unwrap_or(self.toks[self.toks.len() - 1].clone())
    }

    //FIX: I don't like these defaults
    //
    pub(super) fn peek_behind(&self, dest: usize) -> SpannedToken {
        self.toks
            // This subtraction is always from a controlled area so it can't fail unless external
            // usage is harmful.
            .get(self.pos - dest)
            .map(|st| st.clone())
            .unwrap_or(self.toks[self.toks.len() - 1].clone())
    }

    pub(super) fn advance_tok(&mut self) -> Token {
        if self.pos >= self.toks.len() {
            return self.toks[self.toks.len() - 1].tok;
        }

        let t = self.toks[self.pos].tok;
        self.pos += 1;
        t
    }

    pub(super) fn peek_span(&self) -> SourceSpan {
        let t = self.toks[self.pos].span.clone();
        t
    }

    pub(super) fn advance_span(&mut self) -> SourceSpan {
        if self.pos >= self.toks.len() {
            return self.toks[self.toks.len() - 1].span;
        }

        let t = self.toks[self.pos].span.clone();
        self.pos += 1;
        t
    }

    fn advance(&mut self) -> SpannedToken {
        // If it made it here there has to be at least ONE token inside so this is fine
        if self.pos >= self.toks.len() {
            return self.toks[self.toks.len() - 1].clone();
        }

        let t = self.toks[self.pos].clone();
        self.pos += 1;
        t
    }
}
