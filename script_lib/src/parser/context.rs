use common::{color, intern::Intern, keywords, metadata::ChernMetadata, reporter, symbols::Span};

use crate::{
    algo,
    parser::error::{Branch, Diagnostic},
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
const C_BRANCH_VAR_TYPE_SET: u64 = C_BASE_EXIT_SET | token::O_BRACKET | token::HASH_SYMBOL;

const A_BRANCH_VAR_TYPE_SET: u64 = A_BASE_EXIT_SET | token::COLON;

// Probably shouldn't account for hash symbol since it is not apart of the loop
const C_BRANCH_VAR_COND_SET: u64 = C_BASE_EXIT_SET | token::HASH_SYMBOL | token::C_CURLY_BRACKET;
const A_BRANCH_VAR_COND_SET: u64 = A_BASE_EXIT_SET | token::COLON;

const C_BRANCH_VAR_ARGS_SET: u64 = C_BASE_EXIT_SET | token::HASH_SYMBOL | token::C_CURLY_BRACKET;
const A_BRANCH_VAR_ARGS_SET: u64 = A_BASE_EXIT_SET | token::COLON;

//TODO: Find out what tuning works best for these if they are going to stay.
const C_BRANCH_VAR_FUNC_SET: u64 = C_BASE_EXIT_SET | token::C_PAREN;
const A_BRANCH_VAR_FUNC_SET: u64 = A_BASE_EXIT_SET | token::C_BRACKET;

#[derive(Debug)]
pub(super) struct Context<'a> {
    metadata: &'a ChernMetadata,
    pub(super) toks: &'a [SpannedToken],
    pub(super) pos: usize,
    pub(super) err_vec: Vec<Diagnostic>,
}

impl<'a> Context<'a> {
    pub(super) fn new(metadata: &'a ChernMetadata, tokens: &'a [SpannedToken]) -> Context<'a> {
        Context {
            metadata,
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
        let found = &self.toks[self.pos];
        self.pos += 1;

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

        let span = self.safely_handle_span(found);

        let ln_data =
            reporter::form_err_diag(&self.metadata.src_bytes, &span, self.metadata.can_color);

        let msg = if let Some(name) = id_opt {
            let msg = format!("(in {branch})\n{bmsg}\"{name}\"{amsg}");

            reporter::standardize_err(&msg, &ln_data, &help)
        } else {
            let msg = format!("(in {branch})\n{bmsg}'{}'{amsg}", found.tok.kind());

            reporter::standardize_err(&msg, &ln_data, &help)
        };

        self.err_vec.push(Diagnostic::new(msg, branch));

        self.recover(branch);

        Err(found.tok)
    }

    // BOF
    /// Intended for basic errors that need little context after
    /// ALWAYS advance before using this or ensure an advance happened before.
    pub(super) fn report_verbose(&mut self, msg: &str, branch: Branch, interner: &Intern) {
        let found = &self.toks[self.pos - 1];

        let help = self
            .try_help(TokenKind::Poison, &found, branch, interner)
            .unwrap_or_default();

        let span = self.safely_handle_span(found);

        let ln_data =
            reporter::form_err_diag(&self.metadata.src_bytes, &span, self.metadata.can_color);

        let base_msg = format!("(in {branch})\n{msg}");

        let msg = reporter::standardize_err(&base_msg, &ln_data, &help);

        self.recover(branch);

        let diag = Diagnostic::new(msg, branch);

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

            let ln_data =
                reporter::form_err_diag(&self.metadata.src_bytes, &span, self.metadata.can_color);

            let help = self
                .try_help(expected, &found, branch, interner)
                .unwrap_or_default();

            let msg = if let Some(id_str) = id_str_opt {
                let base_msg = format!(
                    "(in {branch})\n{bmsg}{} \"{id_str}\"{amsg}",
                    found.tok.kind()
                );

                reporter::standardize_err(&base_msg, &ln_data, &help)
            } else {
                let base_msg = format!("(in {branch})\n{bmsg}'{}'{amsg}", found.tok.kind());

                reporter::standardize_err(&base_msg, &ln_data, &help)
            };

            self.err_vec.push(Diagnostic::new(msg, branch));

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
            reporter::form_err_diag(&self.metadata.src_bytes, &span, self.metadata.can_color);

        let base_msg = format!("(in {branch})\nExpected {emsg}, found {fmsg}");

        let msg = reporter::standardize_err(&base_msg, &ln_data, &help);

        self.recover(branch);

        let diag = Diagnostic::new(msg, branch);

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
            Branch::Neutral => (C_STMT_NEUTRAL_SET, A_BASE_EXIT_SET),
            Branch::Searching => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
            Branch::Bind => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
            Branch::Var => (C_BRANCH_VAR_SET, A_BRANCH_VAR_SET),
            Branch::VarType => (C_BRANCH_VAR_TYPE_SET, A_BRANCH_VAR_TYPE_SET),
            Branch::Cond => (C_BRANCH_VAR_COND_SET, A_BRANCH_VAR_COND_SET),
            Branch::VarFuncArgs => (C_BRANCH_VAR_FUNC_SET, A_BRANCH_VAR_FUNC_SET),
            Branch::VarTypeArgs => (C_BRANCH_VAR_ARGS_SET, A_BRANCH_VAR_ARGS_SET),
            Branch::Nest => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
            //TODO: Tune these sets
            Branch::NestType => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
            Branch::NestEnum => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
            Branch::Complex => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
            Branch::Override => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
        }
    }

    //TODO: Give help_model the ability to send help
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

        match branch {
            //FIXME: Currently suggests on any error in neutral so...more branches
            Branch::Neutral => match found.tok {
                Token::Id(id) | Token::Illegal(id) => {
                    let found_bytes = interner.search(id as usize).as_bytes();

                    // Statements and sections are possible so both are tried
                    let similar = algo::fuzzy_match(found_bytes, algo::FuzzyMatch::Stmt)
                        .is_none()
                        .then_some(algo::fuzzy_match(found_bytes, algo::FuzzyMatch::Sect))??;

                    let msg = format!("Found similar section \"{similar}\"");
                    let help = reporter::standardize_help(&msg, self.metadata.can_color);

                    Some(help)
                }
                _ => None,
            },
            Branch::Searching => match found.tok {
                Token::Id(id) | Token::Illegal(id) => {
                    let found_bytes = interner.search(id as usize).as_bytes();

                    // If it's already a valid section name then it won't send false help
                    if keywords::sect_range().contains(&(id as usize)) {
                        return None;
                    };

                    let similar_sect = algo::fuzzy_match(found_bytes, algo::FuzzyMatch::Sect)?;

                    let msg = format!("Found similar section \"{similar_sect}\"");
                    let help = reporter::standardize_help(&msg, self.metadata.can_color);

                    Some(help)
                }
                _ => None,
            },
            Branch::Var => match found.tok {
                Token::OParen if expected == TokenKind::Colon => {
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
            Branch::VarType => match found.tok {
                Token::CAngleBracket if prev_kind == TokenKind::Comma => {
                    let msg = "Was there a trailing ',' or an intended second type?";
                    let help = reporter::standardize_help(msg, self.metadata.can_color);

                    Some(help)
                }
                _ => None,
            },
            Branch::Cond => match found.tok {
                Token::Id(id) if expected == TokenKind::CBracket => {
                    let msg = "Is there a missing comma to separate conditions?";
                    let help = reporter::standardize_help(msg, self.metadata.can_color);

                    Some(help)
                }
                Token::CBracket if prev_kind == TokenKind::Comma => {
                    let msg = "Remove trailing ',' or add a condition";
                    let help = reporter::standardize_help(msg, self.metadata.can_color);

                    Some(help)
                }
                _ => None,
            },
            Branch::NestEnum => match found.tok.kind() {
                TokenKind::Colon => {
                    let msg = "Enums use tuples to hold types";
                    let help = reporter::standardize_help(msg, self.metadata.can_color);

                    Some(help)
                }
                _ => None,
            },
            Branch::VarTypeArgs => match found.tok {
                Token::Id(id) => {
                    let found_bytes = interner.search(id as usize).as_bytes();

                    let similar_arg = algo::fuzzy_match(found_bytes, algo::FuzzyMatch::Arg)?;

                    let help = reporter::standardize_help(
                        &format!("Found similar argument \"{similar_arg}\"",),
                        self.metadata.can_color,
                    );

                    Some(help)
                }
                _ => None,
            },
            _ => None,
        }
    }

    pub(super) fn emit_errors(&self) {
        let (red, nc) = color::get_red(self.metadata.can_color);

        let header_err = format!("{red}error{nc}");

        println!("From path => \"{}\"", self.metadata.path.display());

        for err in &self.err_vec {
            println!("{header_err}: {}", err.msg);
        }

        eprintln!("Reported {} error(s)", self.err_vec.len());
    }

    //WARN: IF ANYTHING HAPPENS TO ERROR MESSAGES REMOVE THIS
    fn safely_handle_span(&self, found: &SpannedToken) -> Vec<Span> {
        if found.tok.kind() == TokenKind::EOF {
            // Minus 2 since we advanced at the beginning
            let start_span = self.toks.get(self.pos - 2).unwrap_or(found).span.clone();
            vec![start_span, found.span.clone()]
        } else {
            vec![found.span.clone()]
        }
    }

    pub(super) fn skip(&mut self, dest: usize) -> () {
        self.pos += dest;
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

    pub(super) fn peek_ahead(&self, dest: usize) -> &SpannedToken {
        &self.toks[self.pos + dest]
    }

    pub(super) fn advance_tok(&mut self) -> Token {
        let t = self.toks[self.pos].tok;
        self.pos += 1;
        t
    }

    pub(super) fn peek_span(&mut self) -> Span {
        let t = self.toks[self.pos].span.clone();
        t
    }

    pub(super) fn advance_span(&mut self) -> Span {
        let t = self.toks[self.pos].span.clone();
        self.pos += 1;
        t
    }

    fn advance(&mut self) -> &SpannedToken {
        let t = &self.toks[self.pos];
        self.pos += 1;
        t
    }
}
