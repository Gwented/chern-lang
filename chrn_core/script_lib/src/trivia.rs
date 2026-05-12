use common::span::Span;

/// Structure to represent data that would not be important to semantic data, and instead useful to
/// external tooling since it preserves positions for characters like new lines and ocmments.
#[derive(Debug, Clone)]
pub struct Trivia {
    pub kind: TriviaKind,
    pub span: Span,
}

impl Trivia {
    pub fn new(kind: TriviaKind, span: Span) -> Trivia {
        Trivia { kind, span }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriviaKind {
    SingleComment,
    MultiComment,
    Tab,
    Whitespace,
    NewLine,
}

impl TriviaKind {
    // IS THE TAB CONTROL
    pub fn is_spacing_no_newline(&self) -> bool {
        match self {
            TriviaKind::Tab | TriviaKind::Whitespace => true,
            TriviaKind::NewLine | TriviaKind::SingleComment | TriviaKind::MultiComment => false,
        }
    }

    pub fn is_spacing(&self) -> bool {
        match self {
            TriviaKind::Tab | TriviaKind::Whitespace | TriviaKind::NewLine => true,
            TriviaKind::SingleComment | TriviaKind::MultiComment => false,
        }
    }

    pub fn is_comment(&self) -> bool {
        match self {
            TriviaKind::Tab | TriviaKind::Whitespace | TriviaKind::NewLine => false,
            TriviaKind::SingleComment | TriviaKind::MultiComment => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentLocation {
    Inline,
    Trailing,
    SingleLine,
}
