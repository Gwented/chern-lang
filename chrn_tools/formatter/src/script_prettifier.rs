//TODO: Remove all saturating offset adds for spanning in funny
use chrn_utils::{id_types::AstId, intern::Intern};
use script_lib::{
    parser::ast::{AbstractVar, AstInfo, Expr, Item, Section, SpannedExpr},
    token::{SpannedToken, Token},
    trivia::{CommentLocation, Trivia, TriviaKind},
};

pub(crate) struct ScriptPrettifier<'a> {
    src_str: &'a str,
    ast_info: &'a AstInfo,
    // items: &'a [Item],
    // sections: &'a [Option<Section>],
    toks: &'a Vec<SpannedToken>,
    interner: &'a Intern,
    trivia: &'a Vec<Trivia>,
    pos: usize,
    indent: usize,
}

impl ScriptPrettifier<'_> {
    pub(crate) fn new<'a>(
        src_str: &'a str,
        ast_info: &'a AstInfo,
        // items: &'a [Item],
        // sections: &'a [Option<Section>],
        toks: &'a Vec<SpannedToken>,
        interner: &'a Intern,
        trivia: &'a Vec<Trivia>,
    ) -> ScriptPrettifier<'a> {
        ScriptPrettifier {
            src_str,
            ast_info,
            // sections,
            toks,
            trivia,
            interner,
            pos: 0,
            indent: 0,
        }
    }

    pub(crate) fn prettify_script(&mut self) -> String {
        let mut fmtted_script = String::new();

        if self.peek_tok() == Token::Def {
            fmtted_script.push_str("@def\n");
            self.advance_tok();
        }

        for sect_opt in self.ast_info.sections() {
            if let Some(sect) = sect_opt {
                match sect {
                    Section::Neutral(ast_ids) => {
                        self.fmt_sect_neutral(&ast_ids, &mut fmtted_script);
                    }
                    Section::Var(ast_ids) => {
                        self.append_leading_trivia(&mut fmtted_script);
                        self.advance_tok();
                        self.append_leading_trivia(&mut fmtted_script);
                        self.advance_tok();
                        fmtted_script.push_str(&format!("\nvar->"));
                        self.fmt_sect_var(&ast_ids, &mut fmtted_script);
                    }
                    Section::Nest(ast_ids) => {
                        todo!()
                        // fmtted_script.push_str(&format!("\nnest->"));
                        // self.increase_indent();
                        // self.fmt_sect_nest(&ast_ids, &mut fmtted_script);
                    }
                    Section::Override(ast_ids) => todo!(),
                    Section::Complex(ast_ids) => todo!(),
                }
            }
            self.decrease_indent();
        }

        if self.peek_tok() == Token::End {
            self.append_leading_trivia(&mut fmtted_script);
            self.advance_tok();
            debug_assert_eq!(self.indent, 0);
            fmtted_script.push_str(&format!("\n@end"));
        }

        println!("{fmtted_script}");
        todo!()
    }

    fn fmt_sect_neutral(&mut self, neutral_ast_ids: &Vec<AstId>, fmtted_script: &mut String) {
        for ast_id in neutral_ast_ids {
            let item = &self.ast_info.items()[ast_id.id as usize];
            match item {
                Item::Var(abs_var) => self.fmt_abs_var(abs_var, fmtted_script),
                Item::Alias(abs_alias) => todo!(),
                _ => unreachable!("Parser broke"),
            }
            dbg!(item);
        }
        // println!("{}", fmtted_script);
        // todo!("COW");
    }

    fn fmt_sect_var(&mut self, var_ast_ids: &Vec<AstId>, fmtted_script: &mut String) {
        for ast_id in var_ast_ids {
            let item = &self.ast_info.items()[ast_id.id as usize];
            match item {
                Item::TypeDef(abs_typedef) => todo!(),
                _ => unreachable!("Parser broke"),
            }
        }
    }

    fn fmt_abs_var(&mut self, abs_var: &AbstractVar, fmtted_script: &mut String) {
        let current = self.peek_spanned().leading_trivia_indices;
        let indent = " ".repeat(self.indent as usize);
        if self.trivia[current.start..current.end]
            .iter()
            .all(|t| !t.kind.is_comment())
        {
            fmtted_script.push_str(&indent);
        }

        if !abs_var.is_priv {
            self.append_leading_trivia(fmtted_script);
            fmtted_script.push_str("export ");
            self.advance_tok();
        }

        self.append_leading_trivia(fmtted_script);
        fmtted_script.push_str("let ");
        self.advance_tok();

        self.append_leading_trivia(fmtted_script);
        let name = self.interner.search(abs_var.name_id.id as usize);
        fmtted_script.push_str(&format!("{name} "));
        self.advance_tok();

        self.append_leading_trivia(fmtted_script);
        fmtted_script.push_str("= ");
        self.advance_tok();

        self.append_leading_trivia(fmtted_script);
        let expr_str = self.fmt_expr(&abs_var.spanned_expr);
        fmtted_script.push_str(&expr_str);
        self.advance_tok();

        let next = self.peek_spanned().leading_trivia_indices;
        if !self.peek_tok().kind().is_terminator()
            && self.trivia[next.start..next.end]
                .iter()
                .all(|t| !t.kind.is_comment())
        {
            fmtted_script.push('\n');
        }

        dbg!(next);

        println!("\n-----------------\n");
        // println!("{}", &fmtted_script);
        // todo!()
    }

    // Whitespace boolean?
    fn append_leading_trivia(&mut self, fmtted_script: &mut String) {
        let leading_span = self.peek_spanned().leading_trivia_indices;
        let indent = " ".repeat(self.indent as usize);
        // Means it didn't actually have anything before it
        if leading_span.start == leading_span.end {
            return;
        }

        let leading_trivias = &self.trivia[leading_span.start..leading_span.end];

        //TODO: Comment semantic analysis :)

        for (i, trivia) in leading_trivias.iter().enumerate() {
            let data = match trivia.kind {
                TriviaKind::SingleComment => {
                    let comment = match Self::classify_comment(leading_trivias, i) {
                        CommentLocation::Trailing => format!(
                            " {}\n{indent}",
                            &self.src_str[trivia.span.start..=trivia.span.end]
                        ),
                        CommentLocation::SingleLine => {
                            format!(
                                "{}\n{indent}",
                                &self.src_str[trivia.span.start..=trivia.span.end]
                            )
                        }
                        _ => unreachable!("No"),
                    };

                    comment
                }
                TriviaKind::MultiComment => {
                    let comment = match Self::classify_comment(leading_trivias, i) {
                        CommentLocation::Trailing => format!(
                            " {}\n{indent}",
                            &self.src_str[trivia.span.start..=trivia.span.end]
                        ),
                        CommentLocation::SingleLine => {
                            format!(
                                "{}\n{indent}",
                                &self.src_str[trivia.span.start..=trivia.span.end]
                            )
                        }
                        CommentLocation::Inline => {
                            todo!()
                        }
                    };

                    comment
                }
                // TriviaKind::NewLine => {
                //     if leading_trivias.len() + 1 > leading_trivias.len() {
                //         continue;
                //     }
                //
                //     let next_kind_opt = leading_trivias.get(i + 1).map(|t| t.kind);
                //     let prev_kind_opt = leading_trivias.get(i - 1).map(|t| t.kind);
                //
                //     if next_kind_opt == Some(TriviaKind::SingleComment)
                //         || next_kind_opt == Some(TriviaKind::MultiComment)
                //             && prev_kind_opt != Some(TriviaKind::NewLine)
                //     {
                //         "\n".into()
                //     } else {
                //         continue;
                //     }
                // }
                _ => continue,
            };

            fmtted_script.push_str(&data);
        }

        // if let Some(trivia) = leading_trivias.iter().last() {
        //     match trivia.kind {
        //         TriviaKind::SingleComment => ,
        //         |TriviaKind::MultiComment => todo!(),
        //         TriviaKind::Tab => todo!(),
        //         TriviaKind::Whitespace => todo!(),
        //         TriviaKind::NewLine => todo!(),
        //     }
    }
    // }

    fn has_code_before(leading_trivias: &[Trivia], idx: usize) -> bool {
        for t in leading_trivias[..idx].iter().rev() {
            if t.kind == TriviaKind::NewLine {
                return false;
            }
        }

        true
    }

    fn has_code_after(leading_trivias: &[Trivia], idx: usize) -> bool {
        for t in &leading_trivias[idx..] {
            if t.kind == TriviaKind::NewLine {
                return false;
            }
        }

        true
    }

    // The probability model.
    fn classify_comment(leading_trivias: &[Trivia], idx: usize) -> CommentLocation {
        match leading_trivias[idx].kind {
            TriviaKind::SingleComment => {
                if Self::has_code_before(leading_trivias, idx) {
                    CommentLocation::Trailing
                } else {
                    CommentLocation::SingleLine
                }
            }
            TriviaKind::MultiComment => {
                let has_code_before = Self::has_code_before(leading_trivias, idx);
                let has_code_after = Self::has_code_after(leading_trivias, idx);

                if idx == 0 || idx + 1 >= leading_trivias.len() {
                    return CommentLocation::Inline;
                }

                let prev_kind = leading_trivias[idx - 1].kind;
                let next_kind = leading_trivias[idx + 1].kind;

                let prev_prev_kind =
                    if idx.checked_sub(2).is_some() && idx - 2 < leading_trivias.len() {
                        leading_trivias.get(idx - 2).map(|t| t.kind)
                    } else {
                        None
                    };

                let next_next_kind = if idx + 2 < leading_trivias.len() {
                    leading_trivias.get(idx + 2).map(|t| t.kind)
                } else {
                    None
                };

                match (prev_prev_kind, next_next_kind) {
                    (Some(prev_prev_kind), Some(next_next_kind)) => {
                        dbg!(prev_prev_kind, prev_kind, next_kind, next_next_kind);
                        if prev_prev_kind == TriviaKind::NewLine
                            && prev_kind.is_spacing_no_newline()
                            && next_kind.is_spacing_no_newline()
                            && next_next_kind == TriviaKind::NewLine
                        {
                            panic!("Sgnle");
                            return CommentLocation::SingleLine;
                        } else if prev_prev_kind == TriviaKind::NewLine
                            && prev_kind.is_spacing_no_newline()
                        {
                        }
                    }
                    (Some(prev_prev_kind), None) => {
                        todo!("hey")
                    }
                    (None, Some(next_next_kind)) => todo!(),
                    _ => (),
                }

                if prev_kind.is_spacing_no_newline() && next_kind.is_spacing_no_newline() {
                    CommentLocation::Inline
                } else if prev_kind.is_comment() && next_kind.is_spacing_no_newline() {
                    CommentLocation::Trailing
                } else {
                    CommentLocation::SingleLine
                }
            }
            _ => unreachable!(),
        }
    }

    fn fmt_expr(&mut self, sp_expr: &SpannedExpr) -> String {
        match &sp_expr.expr {
            Expr::Integer(_, _)
            | Expr::Float(_, _)
            | Expr::Str(_)
            | Expr::Var(_)
            | Expr::Char(_)
            | Expr::Bool(_) => self.src_str[sp_expr.span.start..sp_expr.span.end + 1].into(),
            Expr::Default(interned_id, spanned_expr) => todo!(),
            Expr::Call(spanned_expr, spanned_exprs) => todo!(),
            Expr::MemberAccess(abstract_member_access) => todo!(),
            Expr::Unary(unary) => todo!(),
            Expr::BinaryExpr { lhs, op, rhs } => todo!(),
        }
    }

    fn increase_indent(&mut self) {
        self.indent += 4;
    }

    fn decrease_indent(&mut self) {
        self.indent -= 4;
    }

    fn peek_spanned(&self) -> SpannedToken {
        let t = self.toks[self.pos].clone();
        t
    }

    //TODO: Maybe add a peek that gets the trivial also associated with the value
    fn peek_tok(&self) -> Token {
        let t = self.toks[self.pos].tok;
        t
    }

    // CONCERNING
    fn peek_behind_tok(&self, distance: usize) -> Option<Token> {
        if self.pos.checked_sub(distance).is_none() {
            return None;
        }

        Some(self.toks[self.pos - distance].tok)
    }

    fn peek_behind_spanned(&self, distance: usize) -> Option<SpannedToken> {
        if self.pos.checked_sub(distance).is_none() {
            return None;
        }

        Some(self.toks[self.pos - distance].clone())
    }

    fn advance_tok(&mut self) -> Token {
        let t = self.peek_tok();
        self.pos += 1;
        t
    }

    fn advance_spanned(&mut self) -> SpannedToken {
        let t = self.toks[self.pos].clone();
        self.pos += 1;
        t
    }
}
