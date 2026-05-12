use common::span::Span;
use script_lib::trivia::CommentLocation;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) struct FormatHir {
    kind: TextType,
    span: Span,
    indent: Option<u32>,
}

impl FormatHir {
    pub(crate) fn new(kind: TextType, span: Span, indent: Option<u32>) -> FormatHir {
        FormatHir { kind, span, indent }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum TextType {
    Text,
    Comment(CommentLocation),
}
