use common::span::Span;
use script_lib::trivia::CommentLocation;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) struct TextHir {
    pub kind: TextType,
    // Maybe u32
    pub indent: usize,
}

impl TextHir {
    pub(crate) fn new(kind: TextType, indent: usize) -> TextHir {
        TextHir { kind, indent }
    }
}

//TEST: Will likely be less specific
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum TextType {
    Def(Span),
    End(Span),
    KW(Span),
    Ident(Span),
    Delimiter(Span),
    Op(Span),
    Text(Span),
    Expr(Span),
    Whitespace,
    Newline,
    Comment(CommentLocation, Span),
}
