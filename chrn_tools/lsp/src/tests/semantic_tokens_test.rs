use crate::backend::SemanticTokenType;
use crate::tests::session::{Session, TempWorkspace};
use std::collections::HashSet;
use tower_lsp::lsp_types::SemanticTokensResult;

#[test]
fn test_semantic_token_type_indices_match_legend() {
    assert_eq!(SemanticTokenType::Keyword.as_u32(), 0, "Keyword");
    assert_eq!(SemanticTokenType::String.as_u32(), 1, "String");
    assert_eq!(SemanticTokenType::Number.as_u32(), 2, "Number");
    assert_eq!(SemanticTokenType::Type.as_u32(), 3, "Type");
    assert_eq!(SemanticTokenType::Function.as_u32(), 4, "Function");
    assert_eq!(SemanticTokenType::Macro.as_u32(), 5, "Macro");
    assert_eq!(SemanticTokenType::Operator.as_u32(), 6, "Operator");
    assert_eq!(SemanticTokenType::Variable.as_u32(), 7, "Variable");
    assert_eq!(SemanticTokenType::Property.as_u32(), 8, "Property");
    assert_eq!(SemanticTokenType::Class.as_u32(), 9, "Class");
    assert_eq!(SemanticTokenType::EnumMember.as_u32(), 10, "EnumMember");
    assert_eq!(SemanticTokenType::Regexp.as_u32(), 11, "Regexp");
    assert_eq!(SemanticTokenType::Comment.as_u32(), 12, "Comment");
}

#[test]
fn test_semantic_token_type_indices_are_unique() {
    let variants = [
        SemanticTokenType::Keyword,
        SemanticTokenType::String,
        SemanticTokenType::Number,
        SemanticTokenType::Type,
        SemanticTokenType::Function,
        SemanticTokenType::Macro,
        SemanticTokenType::Operator,
        SemanticTokenType::Variable,
        SemanticTokenType::Property,
        SemanticTokenType::Class,
        SemanticTokenType::EnumMember,
        SemanticTokenType::Regexp,
        SemanticTokenType::Comment,
    ];
    let indices: HashSet<u32> = variants.iter().map(|v| v.as_u32()).collect();
    assert_eq!(
        indices.len(),
        variants.len(),
        "duplicate SemanticTokenType index detected"
    );
}

/// Semantic tokens are delta-encoded from the start of the *file*, not the start of the
/// script region, so the first token of an embedded document carries the `@def` line.
///
/// A missing `script_start` addition would collapse every token onto the data header.
#[tokio::test(start_paused = true)]
async fn test_semantic_tokens_are_emitted_in_absolute_positions() {
    let workspace = TempWorkspace::new("absolute_semantic_tokens");
    let text = "// data header\n@def\nlet value = 3\n@end\ntrailing: data\n";
    let uri = workspace.write("embedded.chrn", text);

    let mut session = Session::new().await;
    session.open(&uri, text).await;

    let result = session
        .semantic_tokens(&uri)
        .await
        .expect("the document produces semantic tokens");
    let SemanticTokensResult::Tokens(tokens) = result else {
        panic!("the server advertises full-document tokens only");
    };

    let [directive, keyword, ..] = tokens.data.as_slice() else {
        panic!("the fixture produces at least two tokens, got {:?}", tokens.data);
    };

    assert_eq!(
        (directive.delta_line, directive.delta_start, directive.length),
        (1, 0, 4),
        "the first token is `@def` on the second line of the file"
    );
    assert_eq!(
        directive.token_type,
        SemanticTokenType::Macro.as_u32(),
        "`@def` is a macro token"
    );
    assert_eq!(
        (keyword.delta_line, keyword.delta_start, keyword.length),
        (1, 0, 3),
        "`let` follows one line below, at the start of the line"
    );
    assert_eq!(
        keyword.token_type,
        SemanticTokenType::Keyword.as_u32(),
        "`let` is a keyword token"
    );
}
