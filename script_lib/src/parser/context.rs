use std::io::IsTerminal;

use common::{intern::Intern, reporter};

use crate::{
    parser::error::{Branch, Diagnostic},
    token::{SpannedToken, Token, TokenKind},
};
//TODO: A struct that contains something like the branch, and error type instead of params

/// Amount of '-' to print for multiple error separation
const TOTAL_SEPARATORS: usize = 60;

//WARN: Seems fine

// A C programmer got lost.
// const ID: u64 = 1 << 0;
// const LITERAL: u64 = 1 << 1;
// const NUMBER: u64 = 1 << 2;
const O_BRACKET: u64 = 1 << 3;
const C_BRACKET: u64 = 1 << 4;
// const O_CURLY_BRACKET: u64 = 1 << 5;
const C_CURLY_BRACKET: u64 = 1 << 6;
// const QUESTION_MARK: u64 = 1 << 7;
// const EQUALS: u64 = 1 << 8;
// const WALRUS: u64 = 1 << 9;
// const O_ANGLE_BRACKET: u64 = 1 << 10;
// const C_ANGLE_BRACKET: u64 = 1 << 11;
const COMMA: u64 = 1 << 12;
const SLIM_ARROW: u64 = 1 << 13;
// const SLASH: u64 = 1 << 14;
const HASH_SYMBOL: u64 = 1 << 15;
// const DOT_RANGE: u64 = 1 << 16;
// const PERCENT: u64 = 1 << 17;
const COLON: u64 = 1 << 18;
// const O_PAREN: u64 = 1 << 19;
const C_PAREN: u64 = 1 << 20;
// const HYPHEN: u64 = 1 << 21;
// const EXCLAMATION_POINT: u64 = 1 << 22;
// const ASTERISK: u64 = 1 << 23;
// const DOUBLE_QUOTES: u64 = 1 << 24;
// const TILDE: u64 = 1 << 25;
const DOT: u64 = 1 << 26;
// const VERTICAL_BAR: u64 = 1 << 27;
const ILLEGAL: u64 = 1 << 28;
// const POISON: u64 = 1 << 29;
const EOF: u64 = 1 << 30;

// C_ == current. A_ == ahead

// ALL SET LOGIC AND PARSE LOGIC NEED TO WORK WITH EACH OTHER
const C_BASE_EXIT_SET: u64 = EOF | ILLEGAL;
const A_BASE_EXIT_SET: u64 = SLIM_ARROW;

const C_BRANCH_VAR_SET: u64 = C_BASE_EXIT_SET | O_BRACKET;
const A_BRANCH_VAR_SET: u64 = A_BASE_EXIT_SET;

// WARN: NestType should probably be responsible for C_CURLY but maybe not
const C_BRANCH_VAR_TYPE_SET: u64 = C_BASE_EXIT_SET | O_BRACKET | HASH_SYMBOL | C_CURLY_BRACKET;
const A_BRANCH_VAR_TYPE_SET: u64 = A_BASE_EXIT_SET | COLON;

// Probably shouldn't accoutn for hash symbol since it is not apart of the loop
const C_BRANCH_VAR_COND_SET: u64 = C_BASE_EXIT_SET | COMMA | HASH_SYMBOL | C_BRACKET;
const A_BRANCH_VAR_COND_SET: u64 = A_BASE_EXIT_SET | COLON;

const C_BRANCH_VAR_ARGS_SET: u64 = C_BASE_EXIT_SET | HASH_SYMBOL;
const A_BRANCH_VAR_ARGS_SET: u64 = A_BASE_EXIT_SET | COLON;

// Nest branch error coordination needs to be fixed first
const C_BRANCH_NEST_SET: u64 = C_BASE_EXIT_SET | DOT;

const C_BRANCH_NEST_TYPE: u64 = C_BASE_EXIT_SET | DOT;

const C_BRANCH_VAR_FUNC_SET: u64 = C_BASE_EXIT_SET | C_PAREN;
const A_BRANCH_VAR_FUNC_SET: u64 = A_BASE_EXIT_SET | C_BRACKET;

//FIX: Help is broken (As in very bad)
//Or add memory instead of having errors the second one is seen. Or just las error. Or Or :=
#[derive(Debug)]
pub struct Context<'a> {
    original_text: &'a [u8],
    pub(crate) tokens: &'a [SpannedToken],
    pub(crate) pos: usize,
    pub(crate) err_vec: Vec<Diagnostic>,
    can_color: bool,
    //TEST:
    should_leave: bool,
    pub(crate) current_branch: Branch,
    pub(crate) past_toks: [TokenKind; 3],
    //TEST:
}

// Fuzzy find?
// I'm NOT having context switch branches manually. Please.
impl<'a> Context<'a> {
    pub fn new(original_text: &'a [u8], tokens: &'a [SpannedToken]) -> Context<'a> {
        Context {
            original_text,
            tokens,
            pos: 0,
            err_vec: Vec::new(),
            should_leave: false,
            can_color: std::io::stdout().is_terminal(),
            current_branch: Branch::Searching,
            past_toks: [TokenKind::Illegal; 3],
        }
    }

    pub(crate) fn expect_id_verbose(
        &mut self,
        expected: TokenKind,
        bmsg: &str,
        amsg: &str,
        branch: Branch,
        interner: &Intern,
    ) -> Result<u32, Token> {
        let found = &self.tokens[self.pos];
        self.pos += 1;

        // WARN: IF ANYTHING GOES WRONG ADD THE IF STATEMENTS BACK FOR EOF

        let id_opt = match found.token {
            Token::Id(id) | Token::Literal(id) | Token::Number(id) => {
                if found.token.kind() == expected {
                    return Ok(id);
                }

                Some(id)
            }
            _ => None,
        };

        let (ln, col, segment) =
            reporter::form_diagnostic(self.original_text, &found.span, self.can_color);

        let msg = if let Some(id) = id_opt {
            let name_id = interner.search(id as usize);
            format!("(in {branch})\n{bmsg}\"{name_id}\"{amsg}\n|\n|[{ln}:{col}]\n{segment}",)
        } else {
            format!(
                "(in {branch})\n{bmsg}'{}'{amsg}\n|\n|[{ln}:{col}]\n|\n{segment}",
                found.token.kind()
            )
        };

        self.err_vec.push(Diagnostic::new(msg, branch));

        self.should_leave = true;

        self.recover(branch);

        Err(found.token)
    }

    /// Intended for basic errors that need little context after
    /// ALWAYS advance before using this or ensure an advance happened before
    pub(crate) fn report_verbose(&mut self, msg: &str, help: Option<&str>, branch: Branch) {
        let found = &self.tokens[self.pos - 1];

        let help = if let Some(msg) = help {
            reporter::form_help(msg, self.can_color)
        } else {
            "".to_string()
        };

        let (ln, col, segment) =
            reporter::form_diagnostic(self.original_text, &found.span, self.can_color);

        let separator = "-".repeat(TOTAL_SEPARATORS);

        let msg = format!("(in {branch})\n{msg}\n|\n|\n[{ln}:{col}]\n{segment}\n{help}{separator}");

        self.should_leave = true;

        self.recover(branch);

        let report = Diagnostic::new(msg, branch);

        self.err_vec.push(report);
    }

    /// Fully curated version of `expect_basic`
    //TODO: Primitive type recognition for printing all errors

    // Return token based off of it's most probable path?
    pub(crate) fn expect_verbose(
        &mut self,
        expected: TokenKind,
        bmsg: &str,
        amsg: &str,
        help: Option<&str>,
        branch: Branch,
        interner: &Intern,
    ) -> Result<Token, Token> {
        let found = &self.tokens[self.pos];

        self.pos += 1;

        if found.token.kind() != expected {
            let id_opt = match found.token {
                Token::Id(id) | Token::Literal(id) | Token::Number(id) => Some(id),
                _ => None,
            };

            let (ln, col, segment) =
                reporter::form_diagnostic(self.original_text, &found.span, self.can_color);

            let separator = "-".repeat(TOTAL_SEPARATORS);

            let help = if let Some(msg) = help {
                reporter::form_help(msg, self.can_color)
            } else {
                "".to_string()
            };

            let msg = if let Some(id) = id_opt {
                let name = interner.search(id as usize);

                format!(
                    "(in {branch})\n{bmsg}{} \"{name}\"{amsg}\n|\n|\n[{ln}:{col}]\n{segment}\n{help}{separator}",
                    found.token.kind()
                )
            } else {
                format!(
                    "(in {branch})\n{bmsg}'{}'{amsg}\n|\n|\n[{ln}:{col}]\n{segment}\n{help}{separator}",
                    found.token.kind()
                )
            };

            self.err_vec.push(Diagnostic::new(msg, branch));

            self.should_leave = true;

            self.recover(branch);

            return Err(found.token);
        }

        Ok(found.token)
    }

    /// More composable "Expected but found" error.
    /// ALWAYS advance before using this
    /// Expected [emsg], found [fmsg]
    pub(crate) fn report_template(&mut self, emsg: &str, fmsg: &str, branch: Branch) {
        let found = &self.tokens[self.pos - 1];

        let (ln, col, segment) =
            reporter::form_diagnostic(self.original_text, &found.span, self.can_color);

        let separator = "-".repeat(TOTAL_SEPARATORS);

        let msg = format!(
            "(in {branch})\nExpected {emsg}, found {fmsg}\n|\n|[{ln}:{col}]\n{segment}\n{separator}",
        );

        self.should_leave = true;

        self.recover(branch);

        let report = Diagnostic::new(msg, Branch::VarTypeArgs);

        self.err_vec.push(report);
    }

    //TODO: Branch specific behavior
    //WARN: SEEMS FINE MAY REMOVE WARN
    fn recover(&mut self, branch: Branch) {
        let (current_target, next_target) = self.match_anchor(branch);

        if self.peek_kind() != TokenKind::EOF {
            while self.pos < self.tokens.len() + 2
                && (self.peek_kind().to_u64() & current_target) == 0
                && (self.peek_ahead(1).token.kind().to_u64() & next_target) == 0
            {
                self.advance();
            }
        }
    }

    // AM I TO ASSUME YOU CAN'T READ TEMPO?
    fn match_anchor(&self, branch: Branch) -> (u64, u64) {
        match branch {
            Branch::Broken => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
            Branch::Searching => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
            Branch::Bind => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
            Branch::Var => (C_BRANCH_VAR_SET, A_BRANCH_VAR_SET),
            Branch::VarType => (C_BRANCH_VAR_TYPE_SET, A_BRANCH_VAR_TYPE_SET),
            Branch::VarCond => (C_BRANCH_VAR_COND_SET, A_BRANCH_VAR_COND_SET),
            Branch::VarFuncArgs => (C_BRANCH_VAR_FUNC_SET, A_BRANCH_VAR_FUNC_SET),
            Branch::VarTypeArgs => (C_BRANCH_VAR_ARGS_SET, A_BRANCH_VAR_ARGS_SET),
            Branch::Nest => (C_BRANCH_NEST_SET, A_BASE_EXIT_SET),
            Branch::NestType => (C_BRANCH_NEST_TYPE, A_BASE_EXIT_SET),
            Branch::ComplexRules => (C_BASE_EXIT_SET, A_BASE_EXIT_SET),
        }
    }

    //TEST:
    pub(crate) fn exit_if(&mut self, branch: Branch) -> Result<(), Token> {
        if self.should_leave {
            self.recover(branch);
        }

        self.should_leave = false;

        Ok(())
    }

    // Oh my Java
    // TEST:
    pub(crate) fn set_branch(&mut self, branch: Branch) {
        self.current_branch = branch;
    }

    pub(crate) fn rewind(&mut self, dest: usize) {
        self.pos -= dest;
    }

    pub(crate) fn skip(&mut self, dest: usize) -> () {
        self.pos += dest;
    }

    pub(crate) fn peek_tok(&mut self) -> Token {
        self.tokens
            .get(self.pos)
            .map(|t| t.token)
            .unwrap_or(Token::EOF)
    }

    pub(crate) fn peek_ahead(&self, dest: usize) -> &SpannedToken {
        &self.tokens[self.pos + dest]
    }

    pub(crate) fn peek_kind(&self) -> TokenKind {
        self.tokens
            .get(self.pos)
            .map(|t| t.token.kind())
            .unwrap_or(TokenKind::EOF)
    }

    pub(crate) fn advance_tok(&mut self) -> Token {
        let t = self.tokens[self.pos].token;
        self.pos += 1;
        t
    }

    fn advance(&mut self) -> &SpannedToken {
        let t = &self.tokens[self.pos];
        self.pos += 1;
        t
    }
}
