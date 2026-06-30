use std::ops::RangeInclusive;

use compilation::lexer;

use crate::text_hir::TextType;

const W_POS: f32 = 0.8;
const W_TOK: f32 = 0.8;
const RATE: f32 = 5e-3;

pub(crate) struct FormatModel {
    whitespace_ctx: WhitespaceCtx,
    newline_ctx: NewlineCtx,
    pub(crate) window: Vec<TextClass>,
}

pub(crate) struct WhitespaceCtx {
    w_pos: f32,
    weights: [f32; 13],
}

pub(crate) struct NewlineCtx {
    w_pos: f32,
    weights: [f32; 13],
}

// Def(SourceSpan),
// End(SourceSpan),
// Keyword(SourceSpan),
// Ident(SourceSpan),
// Delimiter(SourceSpan),
// Operator(SourceSpan),
// Text(SourceSpan),
// Expr(SourceSpan),
// Whitespace,
// NewLine,
// Comment(Inline)
// Comment(Trailing)
// Comment(SingleLine)

const WS_POS_W: f32 = 0.8;

const WS_DEF_W: f32 = 0.6;
const WS_END_W: f32 = 0.0;
const WS_KW_W: f32 = 0.7;
const WS_IDENT_W: f32 = 0.6;
const WS_DELIM_W: f32 = 0.8;
const WS_OP_W: f32 = 1.0;
const WS_TEXT_W: f32 = 0.6;
const WS_EXPR_W: f32 = 0.8;
const WS_WS_W: f32 = 0.8;
const WS_NEW_LINE_W: f32 = 0.8;
const WS_COMMENT_INLINE_W: f32 = 0.8;
const WS_COMMENT_TRAILING_W: f32 = 0.2;
const WS_COMMENT_SINGLE_LINE_W: f32 = 0.4;

const NL_POS_W: f32 = 0.8;

const NL_DEF_W: f32 = 0.7;
const NL_END_W: f32 = 0.3;
const NL_KW_W: f32 = 0.3;
const NL_IDENT_W: f32 = 0.6;
const NL_DELIM_W: f32 = 0.4;
const NL_OP_W: f32 = 0.3;
const NL_TEXT_W: f32 = 0.6;
const NL_EXPR_W: f32 = 0.43;
const NL_WS_W: f32 = 0.2;
const NL_NEW_LINE_W: f32 = 0.2;
const NL_COMMENT_INLINE_W: f32 = 0.2;
const NL_COMMENT_TRAILING_W: f32 = 0.1;
const NL_COMMENT_SINGLE_LINE_W: f32 = 0.4;

impl FormatModel {
    pub(crate) fn init() -> FormatModel {
        let whitespace_ctx = WhitespaceCtx {
            w_pos: WS_POS_W,
            weights: [
                WS_DEF_W,
                WS_END_W,
                WS_KW_W,
                WS_IDENT_W,
                WS_DELIM_W,
                WS_OP_W,
                WS_TEXT_W,
                WS_EXPR_W,
                WS_WS_W,
                WS_NEW_LINE_W,
                WS_COMMENT_INLINE_W,
                WS_COMMENT_TRAILING_W,
                WS_COMMENT_SINGLE_LINE_W,
            ],
        };

        let newline_ctx = NewlineCtx {
            w_pos: NL_POS_W,
            weights: [
                NL_DEF_W,
                NL_END_W,
                NL_KW_W,
                NL_IDENT_W,
                NL_DELIM_W,
                NL_OP_W,
                NL_TEXT_W,
                NL_EXPR_W,
                NL_WS_W,
                NL_NEW_LINE_W,
                NL_COMMENT_INLINE_W,
                NL_COMMENT_TRAILING_W,
                NL_COMMENT_SINGLE_LINE_W,
            ],
        };

        FormatModel {
            whitespace_ctx,
            newline_ctx,
            window: Default::default(),
        }
    }

    // pub(crate) fn inject(&mut self, idx: usize, text_hir: &[TextHir]) {
    //
    //     dbg!(amt);
    // }
}

/// Let me call my guy
pub(crate) fn should_newline(model: &FormatModel, idx: usize) -> bool {
    let range = create_range(model, idx);
    let slice = &model.window[range];
    let ctx = &model.newline_ctx;
    let w_pos = ctx.w_pos;

    let mut confidence = 0.0;

    for (i, class) in slice.iter().enumerate() {
        // dbg!(class);
        let weight = ctx.weights[(*class) as usize];

        let distance = i.max(idx) - i.min(idx);
        let distance_signal = newline_ctx_distance(ctx, *class, distance);

        let attention_signal = context_attend(slice, *class);

        let res = (distance_signal * w_pos + weight * attention_signal) * RATE;

        confidence += res;
    }

    dbg!(confidence);
    panic!();
    confidence > 0.5
}

fn newline_ctx_distance(newline_ctx: &NewlineCtx, class: TextClass, distance: usize) -> f32 {
    let distance = distance as f32;
    let weight = newline_ctx.weights[class as usize];

    let res = distance * (-0.5 * distance).exp();
    dbg!(res);
    res
}

// How much the current token matters given it's relation to all other tokens and the newline
// context
fn context_attend(window: &[TextClass], class: TextClass) -> f32 {
    let mut relation = 0.0;

    for other in window.iter().copied() {
        relation += class.newline_relation_to(other);
        // dbg!(other);
    }
    // panic!();

    relation
}

// Ideal window of 7 text_types
fn create_range(model: &FormatModel, idx: usize) -> RangeInclusive<usize> {
    let window = &model.window;

    // Looking for 3 behind
    let start = if idx >= 4 {
        idx - 4
    } else {
        let mut i = 3;

        while i > 0 {
            i -= 1;
        }

        i
    };

    // Looking for 4 Ahead
    let mut i = 4;
    while i < idx + 4 && i + 1 < window.len() {
        i += 1;
    }

    let end = i;

    start..=end
}

pub(crate) fn should_whitespace(model: &FormatModel, idx: usize) -> bool {
    todo!()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub(crate) enum TextClass {
    Def = 0,
    End = 1,
    KW = 2,
    Ident = 3,
    Delimiter = 4,
    Op = 5,
    Text = 6,
    Expr = 7,
    Whitespace = 8,
    Newline = 9,
    CommentInline = 10,
    CommentTrailing = 11,
    CommentSingleLine = 12,
}

impl TextClass {
    // Maybe could be a simpler general attention based scale
    pub(crate) fn newline_relation_to(&self, other: TextClass) -> f32 {
        // Given another class, how likely would the current class be to want to new line
        match self {
            TextClass::Def => match other {
                TextClass::Def => 0.1,
                TextClass::End | TextClass::KW | TextClass::Ident | TextClass::Text => 0.9,
                TextClass::Delimiter => 0.6,
                TextClass::Op => 0.3,
                TextClass::Expr => 0.6,
                TextClass::Whitespace => 0.4,
                TextClass::Newline => 0.3,
                TextClass::CommentInline => 0.2,
                TextClass::CommentTrailing => 0.8,
                // Not possible on the same line but no line specific data exists yet so
                TextClass::CommentSingleLine => 0.0,
            },
            TextClass::End => match other {
                TextClass::End => unreachable!("Cannot have > 1 end"),
                _ => 0.2,
            },
            TextClass::KW => match other {
                TextClass::Def => 0.8,
                TextClass::End => todo!(),
                TextClass::KW => 0.1,
                TextClass::Ident => todo!(),
                TextClass::Delimiter => todo!(),
                TextClass::Op => todo!(),
                TextClass::Text => todo!(),
                TextClass::Expr => todo!(),
                TextClass::Whitespace | TextClass::Newline => 0.0,
                TextClass::CommentInline => 0.2,
                TextClass::CommentTrailing => 0.8,
                TextClass::CommentSingleLine => 1.0,
            },
            TextClass::Ident => match other {
                TextClass::Def => 0.4,
                TextClass::End => 0.3,
                TextClass::KW => 0.6,
                TextClass::Ident => 0.2,
                TextClass::Delimiter => 0.8,
                TextClass::Op => 0.15,
                TextClass::Text => 0.2,
                TextClass::Expr => 0.1,
                TextClass::Whitespace | TextClass::Newline => 0.1,
                TextClass::CommentInline => 0.2,
                TextClass::CommentTrailing => 0.4,
                TextClass::CommentSingleLine => 0.0,
            },
            TextClass::Delimiter => todo!(),
            TextClass::Op => todo!(),
            TextClass::Text => todo!(),
            TextClass::Expr => todo!(),
            TextClass::Whitespace | TextClass::Newline => 0.0,
            TextClass::CommentInline => todo!(),
            TextClass::CommentTrailing => match other {
                _ => 1.0,
                // TextClass::Def => 0.8,
                // TextClass::KW => 0.6,
                // TextClass::Ident => todo!(),
                // TextClass::Delimiter => todo!(),
                // TextClass::Op => todo!(),
                // TextClass::Text => todo!(),
                // TextClass::Expr => todo!(),
                // TextClass::Newline => 0.3,
                // TextClass::CommentSingleLine
                // | TextClass::Whitespace
                // | TextClass::CommentInline
                // | TextClass::CommentTrailing
                // | TextClass::End => 0.0,
            },
            TextClass::CommentSingleLine => match other {
                _ => 1.0,
            },
        }
    }
}

pub(crate) fn embed_text_type(text_type: TextType) -> TextClass {
    match text_type {
        TextType::Def(_) => TextClass::Def,
        TextType::End(_) => TextClass::End,
        TextType::KW(_) => TextClass::KW,
        TextType::Ident(_) => TextClass::Ident,
        TextType::Delimiter(_) => TextClass::Delimiter,
        TextType::Op(_) => TextClass::Op,
        TextType::Text(_) => TextClass::Text,
        TextType::Expr(_) => TextClass::Expr,
        TextType::Whitespace => TextClass::Whitespace,
        TextType::Newline => TextClass::Newline,
        TextType::Comment(location, _) => match location {
            lexer::trivia::CommentLocation::Inline => TextClass::CommentInline,
            lexer::trivia::CommentLocation::Trailing => TextClass::CommentTrailing,
            lexer::trivia::CommentLocation::SingleLine => TextClass::CommentSingleLine,
        },
    }
}
