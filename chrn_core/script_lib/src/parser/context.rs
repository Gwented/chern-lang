use chrn_utils::{
    chrn_settings::ChrnSettings,
    fmter::Formattable,
    id_types::InternedId,
    intern::Intern,
    keywords::Keyword,
    source_map::{
        source_diagnostic::{AnnotationKind, DiagnosticLevel, SourceDiagnostic},
        source_region::SourceRegion,
        source_span::SourceSpan,
    },
};

use crate::{
    algo,
    parser::{NeutralBranch, SectionBranch, branch::Branch},
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
pub(super) struct Context<'a> {
    settings: &'a ChrnSettings,
    pub(super) region: &'a SourceRegion,
    toks: &'a [SpannedToken],
    pos: usize,
    pub(super) err_vec: Vec<SourceDiagnostic>,
}

impl<'a> Context<'a> {
    pub(super) fn new(
        settings: &'a ChrnSettings,
        metadata: &'a SourceRegion,
        toks: &'a [SpannedToken],
    ) -> Context<'a> {
        Context {
            settings,
            region: metadata,
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

        let err_ident_opt = match found.tok {
            Token::Id(id) | Token::Str(id) | Token::Integer(id, _) | Token::Float(id, _) => {
                if found.tok.kind() == expected {
                    return Ok(id);
                } else {
                    self.get_err_ident(found.tok, interner)
                }
            }
            t => self.get_err_ident(t, interner),
        };

        let help = self.try_help(expected, &found, branch, interner);

        let core_msg = if let Some(name) = err_ident_opt {
            format!("(in {branch})\n{bmsg}{name}{amsg}")
        } else {
            format!("(in {branch})\n{bmsg}'{}'{amsg}", found.tok.kind())
        };

        self.push_src_diag(&found, core_msg, help);

        self.recover(branch);

        Err(found.tok)
    }

    pub(super) fn push_src_diag(
        &mut self,
        found: &SpannedToken,
        core_msg: String,
        help_opt: Option<String>,
    ) {
        let spans = self.safely_handle_span(&found);

        let is_eof_err = spans.len() == 2;

        let (kind, label) = if is_eof_err {
            (
                AnnotationKind::Secondary,
                "Token before <eof>".to_string().into(),
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
                "Unexpected <eof>".to_string().into(),
            );
        }

        if let Some(inner_help) = help_opt {
            diag_builder = diag_builder.add_help(inner_help);
        }

        self.err_vec.push(diag_builder.build());
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

        let err_ident_opt = self.get_err_ident(found.tok, interner);

        let help = self.try_help(TokenKind::Keyword, &found, branch, interner);

        let core_msg = if let Some(ident) = err_ident_opt {
            format!("(in {branch})\n{bmsg}{ident}{amsg}")
        } else {
            format!("(in {branch})\n{bmsg}'{}'{amsg}", found.tok.kind())
        };

        self.push_src_diag(&found, core_msg, help);

        self.recover(branch);

        Err(found.tok)
    }

    // BOF
    /// Intended for basic errors that need little context after
    /// This must ALWAYS be advanced before usage due to the found token always being assumed to be
    /// the previous token.
    pub(super) fn report_verbose(&mut self, msg: &str, branch: Branch, interner: &Intern) {
        let found = self.peek_behind(1);

        let help = self.try_help(TokenKind::Poison, &found, branch, interner);

        let core_msg = format!("(in {branch})\n{msg}");

        self.push_src_diag(&found, core_msg, help);

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
            let err_ident_opt = self.get_err_ident(found.tok, interner);

            let help = self.try_help(expected, &found, branch, interner);

            let core_msg = if let Some(id_str) = err_ident_opt {
                format!("(in {branch})\n{bmsg}{} {id_str}{amsg}", found.tok.kind())
            } else {
                format!("(in {branch})\n{bmsg}'{}'{amsg}", found.tok.kind())
            };

            self.push_src_diag(&found, core_msg, help);

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

        let help = self.try_help(TokenKind::Poison, &found, branch, interner);

        let core_msg = format!("(in {branch})\nExpected {emsg}, found {fmsg}");

        self.push_src_diag(&found, core_msg, help);

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
    fn try_help(
        &self,
        expected: TokenKind,
        found: &SpannedToken,
        branch: Branch,
        interner: &Intern,
    ) -> Option<String> {
        // Maybe saturating could lead to mis info
        let prev_tok = self.toks.get(self.pos.saturating_sub(2))?.clone();
        let prev_kind = prev_tok.tok.kind();

        let next_kind = self
            .toks
            .get(self.pos + 1)
            .map(|t| t.tok.kind())
            .unwrap_or(TokenKind::Poison);

        match branch {
            Branch::Neutral(neutral_branch) => match neutral_branch {
                NeutralBranch::Let => match found.tok {
                    Token::Keyword(kw) if expected == TokenKind::Id => {
                        let help = format!("Keywords can be escaped with \"e#{}\"", kw.to_fmt());
                        Some(help)
                    }
                    _ => None,
                },
                NeutralBranch::Searching => match found.tok {
                    // Found stray unrecognizable identifier in neutral
                    Token::Id(name_id) | Token::Illegal(name_id) => {
                        let found_bytes = interner.search(name_id).as_bytes();

                        // Statements and sections are possible so both are tried
                        let similar = algo::fuzzy_match(found_bytes, algo::FuzzyMatch::Stmt)
                            .is_none()
                            .then_some(algo::fuzzy_match(found_bytes, algo::FuzzyMatch::Sect))??;

                        let help = format!("Found similar \"{similar}\"");

                        Some(help)
                    }
                    _ => None,
                },
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
                        None
                    }
                    _ => None,
                },
                _ => None,
            },
            Branch::Section(sect_branch) => match sect_branch {
                SectionBranch::Var => match found.tok {
                    Token::Keyword(kw)
                        if expected == TokenKind::Id && next_kind == TokenKind::Colon =>
                    {
                        let help = format!("Keywords can be escaped with \"e#{}\"", kw.to_fmt());
                        Some(help)
                    }
                    Token::Str(id)
                        if expected == TokenKind::Colon && prev_kind == TokenKind::Id =>
                    {
                        let Token::Id(possible_kw_id) = prev_tok.tok else {
                            return None;
                        };

                        let kw = Keyword::try_from_interned_id(possible_kw_id)?;

                        let help = format!(
                            "If this was meant to use the statement `{}`, place this within `neutral`, which is the area before any section is used",
                            kw.to_fmt()
                        );

                        Some(help)
                    }
                    Token::OParen if expected == TokenKind::Colon => {
                        // if let Token::Id(prev_id) = prev_tok.tok {
                        //     panic!("CApi");
                        // }
                        //
                        None
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
                    _ => None,
                },
                SectionBranch::Nest => match found.tok {
                    // If in `nest->` and found, struct|enum [keyword]
                    Token::Keyword(kw) if expected == TokenKind::Id => {
                        if let Token::Keyword(kw) = prev_tok.tok {
                            if kw == Keyword::Struct || kw == Keyword::Enum {
                                let help =
                                    format!("Keywords can be escaped with \"e#{}\"", kw.to_fmt());

                                return Some(help);
                            }
                        }

                        None
                    }
                    // This will not be usable until a keyword token is made
                    Token::Id(id) if expected == TokenKind::Id && next_kind == TokenKind::Str => {
                        let Token::Id(possible_kw_id) = prev_tok.tok else {
                            return None;
                        };

                        let kw = Keyword::try_from_interned_id(possible_kw_id)?;

                        let help = format!(
                            "If this was meant to use the statement `{}`, place this within `neutral`, which is the area before any section was used.",
                            kw.to_fmt()
                        );

                        Some(help)
                    }
                    _ => None,
                },
                // SectionBranch::NestType => todo!(),
                // SectionBranch::NestEnum => todo!(),
                // SectionBranch::Complex => todo!(),
                // SectionBranch::Override => todo!(),
                _ => match found.tok {
                    Token::Id(ident_id) | Token::Illegal(ident_id) => {
                        let found_bytes = interner.search(ident_id).as_bytes();

                        // Maybe this should return None if it directly IS a direct match since it is
                        // just a range check
                        let similar_sect = algo::fuzzy_match(found_bytes, algo::FuzzyMatch::Sect)?;

                        let help = format!("Found similar section \"{similar_sect}\"");
                        Some(help)
                    }
                    _ => None,
                },
            },
            // Branch::Expr => todo!(),
            Branch::Cond => match found.tok {
                Token::Id(id) if expected == TokenKind::CBracket => {
                    let help = "Is there a missing comma to separate conditions?".to_string();
                    Some(help)
                }
                Token::CBracket if prev_kind == TokenKind::Comma => {
                    let help = "Remove trailing ',' or add a condition".to_string();
                    Some(help)
                }
                _ => None,
            },
            Branch::Type => match found.tok {
                Token::CAngleBracket if prev_kind == TokenKind::Comma => {
                    let help = "Was there a trailing ',' ?".to_string();
                    Some(help)
                }
                _ => None,
            },
            // Branch::FuncArgs => todo!(),
            Branch::TypeArgs => match found.tok {
                Token::Id(name_id) => {
                    let found_bytes = interner.search(name_id).as_bytes();
                    let similar_arg = algo::fuzzy_match(found_bytes, algo::FuzzyMatch::Arg)?;
                    let help = format!("Found similar argument \"{similar_arg}\"");

                    Some(help)
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// Helper to reduce boiler-plate of getting an identifier if possible from an error token
    fn get_err_ident(&self, tok: Token, interner: &Intern) -> Option<String> {
        match tok {
            Token::Def => Some("`@def`".to_string()),
            Token::End => Some("`@end`".to_string()),
            Token::Id(name_id)
            | Token::Str(name_id)
            | Token::Integer(name_id, _)
            | Token::Float(name_id, _) => {
                let ident = interner.search(name_id);
                Some(format!("\"{ident}\""))
            }
            Token::Keyword(kw) => Some(format!("`{}`", kw.to_fmt().to_string())),
            Token::Illegal(name_id) => {
                let illegal_msg = interner.search(name_id);
                let new_msg = format!("invalid token \"{illegal_msg}\"");
                Some(new_msg)
            }
            Token::Char(ch) => Some(format!("'{ch}'")),
            _ => None,
        }
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
        if self.pos >= self.toks.len() {
            return self.toks[self.toks.len() - 1].clone();
        }

        let t = self.toks[self.pos].clone();
        self.pos += 1;
        t
    }
}
