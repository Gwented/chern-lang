use crate::backend::SemanticTokenType;
use std::collections::HashSet;

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
