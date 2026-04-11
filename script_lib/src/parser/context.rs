use common::{
    fmter::Formattable,
    intern::Intern,
    keywords::{self, Keyword},
    metadata::{ChernSettings, ModuleMetadata},
    reporter::{
        self,
        diagnostic::{Area, Diagnostic},
    },
    symbols::Span,
};

use crate::{
    algo,
    parser::{NeutralBranch, SectionBranch, branch::Branch},
    types::{
        symbols::SpannedToken,
        token::{self, Token, TokenKind},
    },
};

//NOTE: C_ == current. A_ == ahead

// ALL SET LOGIC AND PARSE LOGIC NEED TO WORK WITH EACH OTHER
// TODO:  Readjust Sets for new behavior

//NOTE: The basic exit sets should ONLY have tokens that will ALWAYS be stopped on.
const C_BASE_EXIT_SET: u64 = token::EOF | token::ILLEGAL;
const A_BASE_EXIT_SET: u64 = token::SLIM_ARROW;

const C_STMT_NEUTRAL_SET: u64 = C_BASE_EXIT_SET | token::ID;

const C_BRANCH_VAR_SET: u64 = C_BASE_EXIT_SET;
const A_BRANCH_VAR_SET: u64 = A_BASE_EXIT_SET | token::COLON;

// WARN: NestType should probably be responsible for C_CURLY but maybe not
const C_BRANCH_TYPE_SET: u64 = C_BASE_EXIT_SET | token::O_BRACKET | token::HASH_SYMBOL;

const A_BRANCH_TYPE_SET: u64 = A_BASE_EXIT_SET | token::COLON;

// Probably shouldn't account for hash symbol since it is not apart of the loop
const C_BRANCH_COND_SET: u64 = C_BASE_EXIT_SET | token::HASH_SYMBOL | token::C_CURLY_BRACKET;
const A_BRANCH_COND_SET: u64 = A_BASE_EXIT_SET | token::COLON;

const C_BRANCH_TYPE_ARGS_SET: u64 = C_BASE_EXIT_SET | token::HASH_SYMBOL | token::C_CURLY_BRACKET;
const A_BRANCH_TYPE_ARGS_SET: u64 = A_BASE_EXIT_SET | token::COLON;

//TODO: Find out what tuning works best for these if they are going to stay.
const C_BRANCH_FUNC_SET: u64 = C_BASE_EXIT_SET | token::C_PAREN;
const A_BRANCH_FUNC_SET: u64 = A_BASE_EXIT_SET | token::C_BRACKET;

#[derive(Debug)]
pub(super) struct Context<'a> {
    settings: &'a ChernSettings,
    mod_metadata: &'a ModuleMetadata,
    toks: &'a [SpannedToken],
    pos: usize,
    pub(super) err_vec: Vec<Diagnostic>,
}

impl<'a> Context<'a> {
    pub(super) fn new(
        settings: &'a ChernSettings,
        mod_metadata: &'a ModuleMetadata,
        tokens: &'a [SpannedToken],
    ) -> Context<'a> {
        Context {
            settings,
            mod_metadata,
            toks: tokens,
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
    ) -> Result<u32, Token> {
        // WARN: IF ANYTHING GOES WRONG ADD THE IF STATEMENTS BACK FOR EOF
        let found = self.advance();

        //TEST: I JUST WANTED TO USE REFERENCES
        let id_opt = match found.tok {
            Token::Id(id) | Token::Str(id) | Token::Integer(id, _) | Token::Float(id, _) => {
                if found.tok.kind() == expected {
                    return Ok(id);
                }

                Some(interner.search(id as usize).to_string())
            }
            Token::Illegal(id) => {
                let illegal_msg = interner.search(id as usize);
                let new_msg = format!("illegal {illegal_msg}");
                Some(new_msg)
            }
            Token::Char(ch) => Some(ch.to_string()),
            _ => None,
        };

        let help = self
            .try_help(expected, &found, branch, interner)
            .unwrap_or_default();

        let span = self.safely_handle_span(&found);

        let ln_data =
            reporter::form_err_diag(&self.mod_metadata.src_bytes, &span, self.settings.can_color);

        let msg = if let Some(name) = id_opt {
            let msg = format!("(in {branch})\n{bmsg}\"{name}\"{amsg}");

            reporter::standardize_err(
                &msg,
                &ln_data,
                &help,
                &self.mod_metadata.path,
                self.settings.can_color,
            )
        } else {
            let msg = format!("(in {branch})\n{bmsg}'{}'{amsg}", found.tok.kind());

            reporter::standardize_err(
                &msg,
                &ln_data,
                &help,
                &self.mod_metadata.path,
                self.settings.can_color,
            )
        };

        // let msg = self.standardize_diag(msg);

        self.err_vec.push(Diagnostic::new(msg, Area::Script));

        self.recover(branch);

        Err(found.tok)
    }

    // BOF
    /// Intended for basic errors that need little context after
    /// ALWAYS advance before using this or ensure an advance happened before.
    // Need Keyword token to actually help more
    pub(super) fn report_verbose(&mut self, msg: &str, branch: Branch, interner: &Intern) {
        let found = &self.toks[self.pos - 1];

        let help = self
            .try_help(TokenKind::Poison, &found, branch, interner)
            .unwrap_or_default();

        let span = self.safely_handle_span(found);

        let ln_data =
            reporter::form_err_diag(&self.mod_metadata.src_bytes, &span, self.settings.can_color);

        let base_msg = format!("(in {branch})\n{msg}");

        let msg = reporter::standardize_err(
            &base_msg,
            &ln_data,
            &help,
            &self.mod_metadata.path,
            self.settings.can_color,
        );

        self.recover(branch);

        let diag = Diagnostic::new(msg, Area::Script);

        self.err_vec.push(diag);
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
        let found = &self.toks[self.pos];
        self.pos += 1;

        if found.tok.kind() != expected {
            let id_str_opt = match found.tok {
                //TODO: Do something with illegal
                Token::Id(id) | Token::Str(id) | Token::Integer(id, _) => {
                    Some(interner.search(id as usize).to_string())
                }
                Token::Illegal(id) => {
                    let illegal_msg = interner.search(id as usize);
                    let new_msg = format!("illegal {illegal_msg}");

                    Some(new_msg)
                }
                Token::Char(ch) => Some(ch.to_string()),
                _ => None,
            };

            let span = self.safely_handle_span(found);

            let ln_data = reporter::form_err_diag(
                &self.mod_metadata.src_bytes,
                &span,
                self.settings.can_color,
            );

            let help = self
                .try_help(expected, &found, branch, interner)
                .unwrap_or_default();

            let msg = if let Some(id_str) = id_str_opt {
                let base_msg = format!(
                    "(in {branch})\n{bmsg}{} \"{id_str}\"{amsg}",
                    found.tok.kind()
                );

                reporter::standardize_err(
                    &base_msg,
                    &ln_data,
                    &help,
                    &self.mod_metadata.path,
                    self.settings.can_color,
                )
            } else {
                let base_msg = format!("(in {branch})\n{bmsg}'{}'{amsg}", found.tok.kind());

                reporter::standardize_err(
                    &base_msg,
                    &ln_data,
                    &help,
                    &self.mod_metadata.path,
                    self.settings.can_color,
                )
            };

            self.err_vec.push(Diagnostic::new(msg, Area::Script));

            self.recover(branch);

            return Err(found.tok);
        }

        Ok(found.tok)
    }

    /// More composable "Expected but found" error.
    /// ALWAYS advance before using this
    /// Expected [emsg], found [fmsg]
    pub(super) fn report_template(
        &mut self,
        emsg: &str,
        fmsg: &str,
        branch: Branch,
        interner: &Intern,
    ) {
        let found = &self.toks[self.pos - 1];

        let help = self
            .try_help(TokenKind::Poison, &found, branch, interner)
            .unwrap_or_default();

        let span = self.safely_handle_span(found);

        let ln_data =
            reporter::form_err_diag(&self.mod_metadata.src_bytes, &span, self.settings.can_color);

        let base_msg = format!("(in {branch})\nExpected {emsg}, found {fmsg}");

        let msg = reporter::standardize_err(
            &base_msg,
            &ln_data,
            &help,
            &self.mod_metadata.path,
            self.settings.can_color,
        );

        self.recover(branch);

        let diag = Diagnostic::new(msg, Area::Script);

        self.err_vec.push(diag);
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
    fn match_branch(&self, branch: Branch) -> (u64, u64) {
        match branch {
            Branch::Broken => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
            Branch::Searching => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
            Branch::Neutral(neutral_branch) => match neutral_branch {
                NeutralBranch::Searching => (C_STMT_NEUTRAL_SET, A_BASE_EXIT_SET),
                NeutralBranch::Bind => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
                NeutralBranch::Alias => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
                NeutralBranch::Const => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
                NeutralBranch::Import => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
            },
            Branch::Section(sect_branch) => match sect_branch {
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

    //TODO: Give help_model the ability to send help
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
            .unwrap_or(TokenKind::EOF);

        match branch {
            Branch::Neutral(neutral_branch) => match neutral_branch {
                // WHAT IF the MODEL predicted if it should ALLOW for something to be parsed AS
                // A SECTION if it LOOKS like one?
                NeutralBranch::Searching => match found.tok {
                    // Found stray unrecognizable identifier in neutral
                    Token::Id(id) | Token::Illegal(id) => {
                        let found_bytes = interner.search(id as usize).as_bytes();

                        // Statements and sections are possible so both are tried
                        let similar = algo::fuzzy_match(found_bytes, algo::FuzzyMatch::Stmt)
                            .is_none()
                            .then_some(algo::fuzzy_match(found_bytes, algo::FuzzyMatch::Sect))??;

                        let msg = format!("Found similar \"{similar}\"");
                        let help = reporter::standardize_help(&msg, self.settings.can_color);

                        Some(help)
                    }
                    _ => None,
                },
                NeutralBranch::Alias => match found.tok {
                    // Alias missing parameters
                    Token::Assign
                        if prev_kind == TokenKind::Id && next_kind != TokenKind::OParen =>
                    {
                        // WHAT AM I AN LLM? AM I REWARD HACKING?
                        let Token::Id(id) = prev_tok.tok else {
                            return None;
                        };

                        let name = interner.search(id as usize);

                        let help_diag = reporter::help_transform(
                            name,
                            &format!("{name}()"),
                            self.settings.can_color,
                        );

                        // It looks weird now
                        let help = reporter::standardize_help(&help_diag, self.settings.can_color);

                        Some(help)
                    }
                    _ => None,
                },
                _ => None,
            },
            Branch::Section(sect_branch) => match sect_branch {
                SectionBranch::Var => match found.tok {
                    Token::Str(id)
                        if expected == TokenKind::Colon && prev_kind == TokenKind::Id =>
                    {
                        let Token::Id(possible_kw_id) = prev_tok.tok else {
                            return None;
                        };

                        let kw = Keyword::try_as_kw(possible_kw_id)?;

                        let msg = format!(
                            "If this was meant to use the statement `{}`, place this within `neutral`, which is the area before any section was used.",
                            kw.to_fmt()
                        );

                        let help = reporter::standardize_help(&msg, self.settings.can_color);

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
                        // let span = Span::new(prev_tok.span.start, found.span.end);
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
                    // This will not be usable until a keyword token is made
                    Token::Id(id) if expected == TokenKind::Id && next_kind == TokenKind::Str => {
                        let Token::Id(possible_kw_id) = prev_tok.tok else {
                            return None;
                        };

                        let kw = Keyword::try_as_kw(possible_kw_id)?;

                        let msg = format!(
                            "If this was meant to use the statement `{}`, place this within `neutral`, which is the area before any section was used.",
                            kw.to_fmt()
                        );

                        let help = reporter::standardize_help(&msg, self.settings.can_color);

                        Some(help)
                    }
                    _ => None,
                },
                // SectionBranch::NestType => todo!(),
                // SectionBranch::NestEnum => todo!(),
                // SectionBranch::Complex => todo!(),
                // SectionBranch::Override => todo!(),
                s => match found.tok {
                    Token::Id(id) | Token::Illegal(id) => {
                        let found_bytes = interner.search(id as usize).as_bytes();

                        // If it's already a valid section name then it won't send false help
                        if keywords::sect_range().contains(&(id as usize)) {
                            return None;
                        };

                        // Maybe this should return None if it directly IS a direct match since it is
                        // just a range check
                        let similar_sect = algo::fuzzy_match(found_bytes, algo::FuzzyMatch::Sect)?;

                        let msg = format!("Found similar section \"{similar_sect}\"");
                        let help = reporter::standardize_help(&msg, self.settings.can_color);

                        Some(help)
                    }
                    _ => None,
                },
            },
            // Branch::Expr => todo!(),
            Branch::Cond => match found.tok {
                Token::Id(id) if expected == TokenKind::CBracket => {
                    let msg = "Is there a missing comma to separate conditions?";
                    let help = reporter::standardize_help(msg, self.settings.can_color);

                    Some(help)
                }
                Token::CBracket if prev_kind == TokenKind::Comma => {
                    let msg = "Remove trailing ',' or add a condition";
                    let help = reporter::standardize_help(msg, self.settings.can_color);

                    Some(help)
                }
                _ => None,
            },
            Branch::Type => match found.tok {
                Token::CAngleBracket if prev_kind == TokenKind::Comma => {
                    let msg = "Was there a trailing ',' or an intended second type?";
                    let help = reporter::standardize_help(msg, self.settings.can_color);

                    Some(help)
                }
                _ => None,
            },
            // Branch::FuncArgs => todo!(),
            Branch::TypeArgs => match found.tok {
                Token::Id(id) => {
                    let found_bytes = interner.search(id as usize).as_bytes();

                    let similar_arg = algo::fuzzy_match(found_bytes, algo::FuzzyMatch::Arg)?;

                    let help = reporter::standardize_help(
                        &format!("Found similar argument \"{similar_arg}\"",),
                        self.settings.can_color,
                    );

                    Some(help)
                }
                _ => None,
            },
            _ => None,
        }
    }

    /// Intended to handle the case where EOF is reached due to errors likely wanting to show the
    /// last token TO EOF, rather than just EOF
    fn safely_handle_span(&self, found: &SpannedToken) -> Vec<Span> {
        if found.tok.kind() == TokenKind::EOF {
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
            .unwrap_or(SpannedToken {
                tok: Token::EOF,
                span: Span::new(self.pos, self.pos),
            })
    }

    pub(super) fn peek_behind(&self, dest: usize) -> SpannedToken {
        self.toks
            .get(self.pos - dest)
            .map(|st| st.clone())
            .unwrap_or(SpannedToken {
                tok: Token::EOF,
                span: Span::new(self.pos, self.pos),
            })
    }

    pub(super) fn advance_tok(&mut self) -> Token {
        let t = self.toks[self.pos].tok;
        self.pos += 1;
        t
    }

    pub(super) fn peek_span(&self) -> Span {
        let t = self.toks[self.pos].span.clone();
        t
    }

    pub(super) fn advance_span(&mut self) -> Span {
        let t = self.toks[self.pos].span.clone();
        self.pos += 1;
        t
    }

    fn advance(&mut self) -> SpannedToken {
        let t = self.toks[self.pos].clone();
        self.pos += 1;
        t
    }
}
