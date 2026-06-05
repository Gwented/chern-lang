use chrn_utils::source_map::source_span::SourceSpan;
use lang::trivia::CommentLocation;

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
    Def(SourceSpan),
    End(SourceSpan),
    KW(SourceSpan),
    Ident(SourceSpan),
    Delimiter(SourceSpan),
    Op(SourceSpan),
    Text(SourceSpan),
    Expr(SourceSpan),
    Whitespace,
    Newline,
    Comment(CommentLocation, SourceSpan),
}
