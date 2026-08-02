use crate::backend::{ConfigCompletionCandidate, config_completion_items};
use crate::state::DocumentState;
use chrn_utils::id_types::InternedId;
use chrn_utils::intern::Intern;
use compilation::script_compiler::ScriptCompiler;
use std::sync::Arc;

#[test]
fn nested_config_without_a_member_type_still_offers_member_options() {
    let state = DocumentState::new(
        Arc::new(String::new()),
        Vec::new(),
        Vec::new(),
        Intern::init(),
        0,
        None,
        0,
    );
    let compiler = ScriptCompiler::init(None, chrn_utils::arena::Arena::new());
    let candidate = ConfigCompletionCandidate {
        open: 0,
        close: 1,
        name_start: 0,
        type_id: None,
        is_root: false,
        configured_options: vec![InternedId::new(chrn_utils::intern::INTERNED_IDENTS)],
        configured_members: Vec::new(),
    };

    let labels: Vec<String> = config_completion_items(&state, &compiler, candidate, "")
        .into_iter()
        .map(|item| item.label)
        .collect();

    assert!(labels.iter().any(|label| label == "cases"));
    assert!(labels.iter().any(|label| label == "default_val"));
    assert!(!labels.iter().any(|label| label == "idents"));
}
