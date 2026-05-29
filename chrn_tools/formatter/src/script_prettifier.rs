use crate::{
    fmt_model::{self, FormatModel},
    text_hir::{TextHir, TextType},
};

// Can render be used here?
pub(crate) struct ScriptPrettifier<'a> {
    src_str: &'a str,
    all_text_hir: &'a Vec<TextHir>,
    pos: usize,
}

impl ScriptPrettifier<'_> {
    pub(crate) fn new<'a>(
        src_str: &'a str,
        all_text_hir: &'a Vec<TextHir>,
    ) -> ScriptPrettifier<'a> {
        ScriptPrettifier {
            src_str,
            all_text_hir,
            pos: 0,
        }
    }

    pub(crate) fn fmt_script(&mut self) -> String {
        let mut fmtted_script = String::new();
        let mut model = FormatModel::init();

        for hir in self.all_text_hir {
            model.window.push(fmt_model::embed_text_type(hir.kind));
        }

        for (i, text_hir) in self.all_text_hir.iter().enumerate() {
            dbg!(text_hir);
            dbg!(&fmtted_script);
            match text_hir.kind {
                TextType::Def(span) | TextType::End(span) => {
                    fmtted_script.push_str(&self.src_str[span.range_inclusive_usize()]);
                }
                TextType::KW(span) => {
                    fmtted_script.push_str(&self.src_str[span.range_inclusive_usize()]);
                }
                TextType::Ident(span) => todo!(),
                TextType::Delimiter(span) => todo!(),
                TextType::Op(span) => todo!(),
                TextType::Text(span) => todo!(),
                TextType::Expr(span) => todo!(),
                TextType::Whitespace => {
                    fmtted_script.push(' ');
                }
                TextType::Newline => match self.can_use_newline() {
                    Some(true) => fmtted_script.push('\n'),
                    Some(false) => (),
                    None => {
                        if fmt_model::should_newline(&model, i) {
                            fmtted_script.push('\n');
                        }
                    }
                },
                TextType::Comment(_, span) => {
                    fmtted_script.push_str(&self.src_str[span.range_inclusive_usize()])
                }
            }
        }

        todo!()
    }

    // Having thoughts.
    /// Some(bool) representing approving inserting a new line or not
    /// None representing needing more context
    fn can_use_newline(&self) -> Option<bool> {
        let next_opt = self.peek_ahead(1);
        let prev_opt = self.peek_behind(1);

        match (prev_opt, next_opt) {
            (Some(l_hir), Some(r_hir)) => None,
            (None, Some(hir)) => None,
            (Some(hir), None) => match hir.kind {
                TextType::Def(_) => todo!(),
                TextType::End(_) => todo!(),
                TextType::Ident(_) => todo!(),
                TextType::Delimiter(_) => todo!(),
                TextType::Op(_) => todo!(),
                TextType::Comment(location, span) => Some(false),
                // Whitespace could be false maybe
                TextType::KW(_) => Some(false),
                TextType::Whitespace => Some(false),
                TextType::Text(_) => None,
                TextType::Expr(_) => None,
                TextType::Newline => None,
            },
            (None, None) => Some(false),
        }

        // if let Some(next) = self.peek_ahead(1) {
        //     match next.kind {
        //         TextType::Def(_) => return Some(false),
        //         TextType::End(_) => todo!(),
        //         TextType::Keyword(_) => todo!(),
        //         TextType::Ident(_) => todo!(),
        //         TextType::Punctuation(_) => todo!(),
        //         TextType::Operator(_) => todo!(),
        //         TextType::Text(_) => None,
        //         TextType::Expr(_) => todo!(),
        //         TextType::Whitespace => todo!(),
        //         TextType::NewLine => return Some(false),
        //         TextType::Comment(location, _) => match location {
        //             CommentLocation::Inline => return Some(false),
        //             CommentLocation::Trailing => return Some(true),
        //             CommentLocation::SingleLine => return Some(true),
        //         },
        //     }
        // }
        //
        // None
    }

    // fn increase_indent(&mut self) {
    //     self.indent += 4;
    // }
    //
    // fn decrease_indent(&mut self) {
    //     self.indent -= 4;
    // }

    //TODO: Maybe add a peek that gets the trivial also associated with the value
    fn peek(&self) -> TextHir {
        let t = self.all_text_hir[self.pos];
        t
    }

    fn peek_behind(&self, distance: usize) -> Option<TextHir> {
        if self.pos.checked_sub(distance).is_none()
            || self.pos + distance >= self.all_text_hir.len()
        {
            return None;
        }

        Some(self.all_text_hir[self.pos + distance])
    }

    fn peek_ahead(&self, distance: usize) -> Option<TextHir> {
        if self.pos.checked_add(distance).is_none() {
            return None;
        }

        Some(self.all_text_hir[self.pos + distance])
    }

    fn advance(&mut self) -> TextHir {
        let t = self.peek();
        self.pos += 1;
        t
    }
}
