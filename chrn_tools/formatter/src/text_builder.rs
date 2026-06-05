use chrn_utils::{id_types::AstId, intern::Intern, source_map::source_span::SourceSpan};
use lang::{
    parser::ast::{AbstractVar, AstInfo, Item, Section},
    token::{SpannedToken, Token},
    trivia::{CommentLocation, Trivia, TriviaKind},
};

use crate::text_hir::{TextHir, TextType};

pub(crate) struct TextBuilder<'a> {
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

//TEST:
impl TextBuilder<'_> {
    pub(crate) fn new<'a>(
        src_str: &'a str,
        ast_info: &'a AstInfo,
        // items: &'a [Item],
        // sections: &'a [Option<Section>],
        toks: &'a Vec<SpannedToken>,
        interner: &'a Intern,
        trivia: &'a Vec<Trivia>,
    ) -> TextBuilder<'a> {
        TextBuilder {
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

    pub(crate) fn form_hir(&mut self) -> Vec<TextHir> {
        let mut all_text_hirs: Vec<TextHir> = Vec::new();

        if self.peek_tok() == Token::Def {
            let span = self.advance_spanned().span;

            all_text_hirs.push(TextHir::new(TextType::Def(span), self.indent));
            all_text_hirs.push(TextHir::new(TextType::Newline, self.indent));
        }

        for sect_opt in self.ast_info.sections() {
            if let Some(sect) = sect_opt {
                match sect {
                    Section::Neutral(ast_ids) => {
                        self.increase_indent();
                        self.fmt_sect_neutral(ast_ids, &mut all_text_hirs);
                        self.decrease_indent();
                    }
                    Section::Var(ast_ids) => {
                        // "var"
                        self.append_leading_trivia(&mut all_text_hirs);
                        let span = self.advance_spanned().span;
                        dbg!(self.peek_tok());
                        panic!();
                        all_text_hirs.push(TextHir::new(TextType::KW(span), self.indent));
                        // "->"
                        self.append_leading_trivia(&mut all_text_hirs);
                        let span = self.advance_spanned().span;
                        all_text_hirs.push(TextHir::new(TextType::Op(span), self.indent));

                        println!("{}", &self.src_str[98..=111]);
                        panic!("Huh");

                        dbg!(all_text_hirs);
                        panic!("End");
                        self.fmt_sect_neutral(ast_ids, &mut all_text_hirs);
                        todo!()
                    }
                    Section::Nest(ast_ids) => {
                        todo!()
                    }
                    Section::Override(ast_ids) => todo!(),
                    Section::Complex(ast_ids) => todo!(),
                }
            }
        }

        if self.peek_tok() == Token::End {
            self.append_leading_trivia(&mut all_text_hirs);
            all_text_hirs.push(TextHir::new(TextType::Newline, self.indent));
            let span = self.advance_spanned().span;
            all_text_hirs.push(TextHir::new(TextType::End(span), self.indent));
            debug_assert_eq!(self.indent, 0);
        }

        all_text_hirs
    }

    fn fmt_sect_neutral(&mut self, neutral_ast_ids: &Vec<AstId>, all_text_hirs: &mut Vec<TextHir>) {
        for ast_id in neutral_ast_ids {
            let item = &self.ast_info.items()[ast_id.id as usize];
            let abs_span = self.ast_info.get_sym_span(*ast_id);
            self.append_invalid(abs_span, all_text_hirs);

            match item {
                Item::Var(abs_var) => {
                    self.fmt_abs_var(abs_var, all_text_hirs);
                }
                Item::Alias(abs_alias) => todo!(),
                _ => unreachable!("Parser broke"),
            }
            dbg!(item);
        }
        // println!("{}", fmtted_script);
        // todo!("COW");
    }

    fn fmt_abs_var(&mut self, abs_var: &AbstractVar, all_text_hirs: &mut Vec<TextHir>) {
        // Export
        if !abs_var.is_priv {
            self.append_leading_trivia(all_text_hirs);
            let export_span = self.advance_spanned().span;
            all_text_hirs.push(TextHir::new(TextType::KW(export_span), self.indent));
            all_text_hirs.push(TextHir::new(TextType::Whitespace, self.indent));
        }

        // Let
        self.append_leading_trivia(all_text_hirs);
        let let_span = self.advance_spanned().span;

        all_text_hirs.push(TextHir::new(TextType::KW(let_span), self.indent));
        all_text_hirs.push(TextHir::new(TextType::Whitespace, self.indent));

        // Ident
        self.append_leading_trivia(all_text_hirs);
        let ident_span = self.advance_spanned().span;

        all_text_hirs.push(TextHir::new(TextType::Ident(ident_span), self.indent));
        all_text_hirs.push(TextHir::new(TextType::Whitespace, self.indent));

        // Assign
        self.append_leading_trivia(all_text_hirs);
        let op_span = self.advance_spanned().span;

        all_text_hirs.push(TextHir::new(TextType::Op(op_span), self.indent));
        all_text_hirs.push(TextHir::new(TextType::Whitespace, self.indent));

        // Expr
        self.append_leading_trivia(all_text_hirs);
        // WARN: Suspicious
        let expr_span = self.advance_spanned().span;

        all_text_hirs.push(TextHir::new(TextType::Expr(expr_span), self.indent));
        all_text_hirs.push(TextHir::new(TextType::Newline, self.indent));

        dbg!(all_text_hirs);

        println!("\n-----------------\n");
        // println!("{}", &fmtted_script);
        // todo!()
    }

    // Whitespace boolean?
    fn append_leading_trivia(&mut self, all_text_hirs: &mut Vec<TextHir>) {
        let leading_span = self.peek_spanned().leading_trivia_indices;
        let leading_trivias = &self.trivia[leading_span.start as usize..leading_span.end as usize];

        //TODO: Comment semantic analysis :)

        for (i, trivia) in leading_trivias.iter().enumerate() {
            match trivia.kind {
                TriviaKind::SingleComment => {
                    match Self::classify_comment(leading_trivias, i) {
                        CommentLocation::Trailing => {
                            all_text_hirs.push(TextHir::new(TextType::Whitespace, self.indent));

                            let comment = TextType::Comment(CommentLocation::Trailing, trivia.span);
                            all_text_hirs.push(TextHir::new(comment, self.indent));

                            all_text_hirs.push(TextHir::new(TextType::Newline, self.indent));
                        }
                        CommentLocation::SingleLine => {
                            all_text_hirs.push(TextHir::new(TextType::Whitespace, self.indent));
                            let comment =
                                TextType::Comment(CommentLocation::SingleLine, trivia.span);
                            all_text_hirs.push(TextHir::new(comment, self.indent));

                            all_text_hirs.push(TextHir::new(TextType::Newline, self.indent));
                        }
                        CommentLocation::Inline => unreachable!("Single comment logic failed"),
                    };
                }
                TriviaKind::MultiComment => {
                    match Self::classify_comment(leading_trivias, i) {
                        CommentLocation::Trailing => {
                            all_text_hirs.push(TextHir::new(TextType::Whitespace, self.indent));

                            let comment = TextType::Comment(CommentLocation::Trailing, trivia.span);
                            all_text_hirs.push(TextHir::new(comment, self.indent));

                            all_text_hirs.push(TextHir::new(TextType::Newline, self.indent));
                        }
                        CommentLocation::SingleLine => {
                            let comment =
                                TextType::Comment(CommentLocation::SingleLine, trivia.span);
                            all_text_hirs.push(TextHir::new(comment, self.indent));

                            all_text_hirs.push(TextHir::new(TextType::Newline, self.indent));
                        }
                        CommentLocation::Inline => {
                            let comment = TextType::Comment(CommentLocation::Inline, trivia.span);
                            all_text_hirs.push(TextHir::new(comment, self.indent));

                            all_text_hirs.push(TextHir::new(TextType::Whitespace, self.indent));
                        }
                    };
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

    // So just append anything from self.pos -> abstract's span
    // Need to track where invalid is within ast in some form
    fn append_invalid(&mut self, abs_span: SourceSpan, all_text_hirs: &mut Vec<TextHir>) {
        // while self.peek_spanned().span.start < abs_span.start {
        //     panic!("Illegal");
        // }
    }

    // The probability model.
    // NOTE: Will probably build a higher level understanding of lines with an official line mapper
    // beyond the reporter
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
                todo!();
                // let has_code_before = Self::has_code_before(leading_trivias, idx);
                // let has_code_after = Self::has_code_after(leading_trivias, idx);

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
                        if prev_prev_kind == TriviaKind::Newline
                            && prev_kind.is_spacing_no_newline()
                            && next_kind.is_spacing_no_newline()
                            && next_next_kind == TriviaKind::Newline
                        {
                            panic!("Sgnle");
                            return CommentLocation::SingleLine;
                        } else if prev_prev_kind == TriviaKind::Newline
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

    fn has_code_before(leading_trivias: &[Trivia], idx: usize) -> bool {
        for t in leading_trivias[..idx].iter().rev() {
            if t.kind == TriviaKind::Newline {
                return false;
            }
        }

        true
    }

    fn has_code_after(leading_trivias: &[Trivia], idx: usize) -> bool {
        for t in &leading_trivias[idx..] {
            if t.kind == TriviaKind::Newline {
                return false;
            }
        }

        true
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
        if self.pos.checked_sub(distance).is_none() || self.pos + distance >= self.toks.len() {
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
