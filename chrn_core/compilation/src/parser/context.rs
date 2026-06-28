// Static access help messages
use chrn_utils::{
    chrn_settings::ChrnSettings,
    id_types::InternedId,
    intern::Intern,
    source_map::{
        source_diagnostic::{
            AnnotationKind, DiagnosticLevel, SourceDiagnostic, SourceDiagnosticBuilder,
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
    parser::{NeutralBranch, SectionBranch, branch::Branch, parse_fmt},
    token::{self, SpannedToken, Token, TokenKind},
};

// C_ == current. A_ == ahead

// ALL SET LOGIC AND PARSE LOGIC NEED TO WORK WITH EACH OTHER
//TODO: Most optimal solution for this is to act off of only section context, so not as granular

//NOTE: The basic exit sets should ONLY have tokens that will ALWAYS be stopped on.
const C_BASE_EXIT_SET: u64 = token::EOF | token::ILLEGAL | token::KEYWORD;
const A_BASE_EXIT_SET: u64 = token::SLIM_ARROW;

const C_STMT_NEUTRAL_SET: u64 = C_BASE_EXIT_SET /*| token::Keyword*/ ;

const C_BRANCH_VAR_SET: u64 = C_BASE_EXIT_SET;
const A_BRANCH_VAR_SET: u64 = A_BASE_EXIT_SET | token::COLON;

// WARN: NestType should probably be responsible for C_CURLY but maybe not
const C_BRANCH_TYPE_SET: u64 = C_BASE_EXIT_SET | token::O_BRACKET | token::HASH_SYMBOL;

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
    settings: &'a ChrnSettings,
    pub(super) region: &'a SourceRegion,
    toks: &'a [SpannedToken],
    pos: usize,
    pub(super) err_vec: Vec<SourceDiagnostic>,
}

impl<'a> ParserContext<'a> {
    pub(super) fn new(
        settings: &'a ChrnSettings,
        region: &'a SourceRegion,
        toks: &'a [SpannedToken],
    ) -> ParserContext<'a> {
        ParserContext {
            settings,
            region,
            toks,
            pos: 0,
            err_vec: Vec::new(),
        }
    }

    /// Returns an interned name id on success and the failed token on error.
    pub(super) fn expect_id_verbose(
        &mut self,
        expected: TokenKind,
        bmsg: &str,
        amsg: &str,
        branch: Branch,
        interner: &Intern,
    ) -> Result<InternedId, Token> {
        // WARN: IF ANYTHING GOES WRONG ADD THE IF STATEMENTS BACK FOR EOF
        let found = self.advance();

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

        let core_msg = format!("(in {branch})\n{bmsg}{fmtted_tok}{amsg}");

        let builder = self.create_diag_builder(&found, core_msg);
        let builder = self.try_assistance(builder, expected, &found, branch, interner);

        self.err_vec.push(builder.build());

        self.recover(branch);

        Err(found.tok)
    }

    pub(super) fn create_diag_builder(
        &mut self,
        found: &SpannedToken,
        core_msg: String,
    ) -> SourceDiagnosticBuilder {
        let spans = self.safely_handle_span(&found);

        // A little odd...
        let terminator_str: &str = match found.tok {
            Token::EOF => "<eof>",
            Token::End => "`@end`",
            _ => "",
        };

        let (kind, label) = if !terminator_str.is_empty() {
            (
                AnnotationKind::Secondary,
                format!("Token before {terminator_str}").into(),
            )
        } else {
            (AnnotationKind::Primary, None)
        };

        let mut diag_builder =
            SourceDiagnostic::builder(DiagnosticLevel::Error, core_msg, self.region.path_id)
                .add_annotation(spans[0], kind, label);

        // Meaning EOF error
        if spans.len() == 2 {
            diag_builder = diag_builder.add_annotation(
                spans[1],
                AnnotationKind::Primary,
                format!("Unexpected {terminator_str}").into(),
            );
        }

        diag_builder
    }

    /// Returns an interned name id on success and the failed token on error.
    pub(super) fn expect_kw_verbose(
        &mut self,
        bmsg: &str,
        amsg: &str,
        branch: Branch,
        interner: &Intern,
    ) -> Result<Keyword, Token> {
        // WARN: IF ANYTHING GOES WRONG ADD THE IF STATEMENTS BACK FOR EOF
        // HIGHLIY SUSPICIOUS
        let found = self.advance();

        if let Token::Keyword(kw) = found.tok {
            return Ok(kw);
        }

        let fmtted_tok = parse_fmt::fmt_tok(found.tok, interner);

        // Well maybe fmt_tok should be a method at this point, or at least an associated function
        let core_msg = format!("(in {branch})\n{bmsg}{fmtted_tok}{amsg}");

        let builder = self.create_diag_builder(&found, core_msg);

        let builder = self.try_assistance(builder, TokenKind::Keyword, &found, branch, interner);
        self.err_vec.push(builder.build());

        self.recover(branch);

        Err(found.tok)
    }

    // BOF
    /// Intended for basic errors that need little context after
    /// This must ALWAYS be advanced before usage due to the found token always being assumed to be
    /// the previous token.
    pub(super) fn report_verbose(&mut self, msg: &str, branch: Branch, interner: &Intern) {
        let found = self.peek_behind(1);

        let core_msg = format!("(in {branch})\n{msg}");

        let builder = self.create_diag_builder(&found, core_msg);
        let builder = self.try_assistance(builder, TokenKind::Poison, &found, branch, interner);

        self.err_vec.push(builder.build());

        self.recover(branch);
    }

    /// Returns the found token on success and failure.
    // TODO:  Maybe lazily evaluate since searching the interner by default is a weird performance
    // hit. Probably.
    pub(super) fn expect_verbose(
        &mut self,
        expected: TokenKind,
        bmsg: &str,
        amsg: &str,
        branch: Branch,
        interner: &Intern,
    ) -> Result<Token, Token> {
        let found = self.advance();

        if found.tok.kind() != expected {
            let fmtted_tok = parse_fmt::fmt_tok(found.tok, interner);

            let core_msg = format!("(in {branch})\n{bmsg}{fmtted_tok}{amsg}");

            let builder = self.create_diag_builder(&found, core_msg);
            let builder = self.try_assistance(builder, expected, &found, branch, interner);
            self.err_vec.push(builder.build());

            self.recover(branch);

            return Err(found.tok);
        }

        Ok(found.tok)
    }

    /// More composable "Expected but found" error.
    /// This must ALWAYS be advanced before usage due to the found token always being assumed to be
    /// the previous token.
    /// Expected [emsg], found [fmsg]
    pub(super) fn report_template(
        &mut self,
        emsg: &str,
        fmsg: &str,
        branch: Branch,
        interner: &Intern,
    ) {
        let found = &self.peek_behind(1);

        let core_msg = format!("(in {branch})\nExpected {emsg}, found {fmsg}");
        let builder = self.create_diag_builder(&found, core_msg);
        let builder = self.try_assistance(builder, TokenKind::Poison, &found, branch, interner);
        self.err_vec.push(builder.build());

        self.recover(branch);
    }

    fn recover(&mut self, branch: Branch) {
        let (current_targets, next_targets) = self.match_branch(branch);

        if self.peek_kind() != TokenKind::EOF {
            while self.pos < self.toks.len() + 2
                && (self.peek_kind().to_u64() & current_targets) == 0
                && (self.peek_ahead(1).tok.kind().to_u64() & next_targets) == 0
            {
                self.advance();
            }
        }
    }

    // AM I TO ASSUME YOU CANNOT READ TEMPO?
    // Yes. You may.
    fn match_branch(&self, branch: Branch) -> (u64, u64) {
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
                SectionBranch::Nest => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
                SectionBranch::NestType => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
                SectionBranch::NestEnum => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
                SectionBranch::Complex => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
                SectionBranch::Override => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
            },
            Branch::Expr => (C_BRANCH_COND_SET, A_BRANCH_COND_SET),
            Branch::Cond => (C_BRANCH_COND_SET, A_BRANCH_COND_SET),
            Branch::Type => (C_BRANCH_TYPE_SET, A_BRANCH_TYPE_SET),
            Branch::FuncArgs => (C_BRANCH_FUNC_SET, A_BRANCH_FUNC_SET),
            Branch::TypeArgs => (C_BRANCH_TYPE_ARGS_SET, A_BRANCH_TYPE_ARGS_SET),
        }
    }

    // TODO: Make a helper reporter so something like can_color doesn't need to be re-entered
    // everytime
    /// Checks available known branching to where a help message can be sent
    fn try_assistance(
        &self,
        mut builder: SourceDiagnosticBuilder,
        expected: TokenKind,
        found: &SpannedToken,
        branch: Branch,
        interner: &Intern,
    ) -> SourceDiagnosticBuilder {
        // ) -> (AnnotationKind, Option<String>) {
        // Maybe saturating could lead to mis info
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
                        builder.add_note("Only `alias` parameters can specify types, all others are inferred".to_string())
                    }
                    _ => builder,
                },
                NeutralBranch::Searching => match found.tok {
                    // Found stray unrecognizable identifier in neutral
                    Token::Id(name_id) | Token::Illegal(name_id) => {
                        let found_bytes = interner.search(name_id).as_bytes();

                        // Statements and sections are possible so both are tried
                        let similar_opt =
                            algo::fuzzy_match_with_fmtted(found_bytes, FuzzyMatch::Stmt)
                                .is_none()
                                .then_some(algo::fuzzy_match_with_fmtted(
                                    found_bytes,
                                    algo::FuzzyMatch::Sect,
                                ));

                        // ???????

                        // Uh huh
                        // Ok
                        if let Some(Some((similar_vec, fmtted_ty))) = similar_opt {
                            let help = Self::fmt_helps(
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
                SectionBranch::Nest => match found.tok {
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
                    // This will not be usable until a keyword token is made
                    Token::Id(id) if expected == TokenKind::Id && next_kind == TokenKind::Str => {
                        let Token::Id(possible_kw_id) = prev_tok.tok else {
                            return builder;
                        };

                        if let Some(kw) = Keyword::try_from_interned_id(possible_kw_id) {
                            let help = format!(
                                "If this was meant to use the statement `{}`, place this within `neutral`, which is the area before any section was used.",
                                kw.to_fmt()
                            );

                            return builder.add_help(help);
                        };

                        builder
                    }
                    _ => builder,
                },
                // SectionBranch::NestType => todo!(),
                // SectionBranch::NestEnum => todo!(),
                SectionBranch::Complex => match found.tok {
                    Token::StaticAccess if expected == TokenKind::OCurlyBracket => {
                        //NOTE: Not sure if this should stick
                        // Also very normal sized message.
                        let help =
                            "Static access is not permitted within configuration declarations.\n  If this was a module namespace, declare this in it's module of origin.\n  If this was a type namespace, this must be defined inside the configuration itself using available syntax."
                                .to_string();

                        builder.add_note(help)
                    }
                    _ => builder,
                },
                // SectionBranch::Override => todo!(),
                _ => match found.tok {
                    Token::Id(ident_id) | Token::Illegal(ident_id) => {
                        let found_bytes = interner.search(ident_id).as_bytes();

                        // Maybe this should return None if it directly IS a direct match since it is
                        // just a range check
                        let similar_opt =
                            algo::fuzzy_match_with_fmtted(found_bytes, algo::FuzzyMatch::Sect);

                        if let Some((similar_vec, fmtted_ty)) = similar_opt {
                            let help = Self::fmt_helps(
                                &similar_vec,
                                &format!("Found similar {fmtted_ty}"),
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
            Branch::TypeArgs => match found.tok {
                Token::Id(name_id) => {
                    let found_bytes = interner.search(name_id).as_bytes();
                    let similar_opt =
                        algo::fuzzy_match_with_fmtted(found_bytes, algo::FuzzyMatch::Directive);

                    if let Some((similar_vec, fmtted_ty)) = similar_opt {
                        let help = Self::fmt_helps(
                            &similar_vec,
                            &format!("Found similar {fmtted_ty}"),
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

        builder
    }

    fn fmt_helps(found: &[&str], header: &str, quote: &str) -> String {
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
    pub(super) fn peek_behind(&self, dest: usize) -> SpannedToken {
        self.toks
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
