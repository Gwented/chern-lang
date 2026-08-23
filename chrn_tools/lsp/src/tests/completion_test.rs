use crate::backend::{ConfigCompletionCandidate, config_completion_items};
use crate::state::DocumentState;
use crate::tests::session::{Session, TempWorkspace, position_of};
use chrn_utils::id_types::InternedId;
use chrn_utils::intern::Intern;
use compilation::script_compiler::ScriptCompiler;
use std::sync::Arc;
use tower_lsp::lsp_types::CompletionResponse;

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

/// An invoked completion inside the script section offers the language keywords and
/// section markers. Completion is refused in serialized data, so this also confirms the
/// request is being classified as script.
#[tokio::test(start_paused = true)]
async fn completion_in_the_script_section_offers_keywords() {
    let workspace = TempWorkspace::new("script_completion");
    let text = "let flag = 3\n";
    let uri = workspace.write("main.chrn", text);

    let mut session = Session::new().await;
    session.open(&uri, text).await;

    let response = session
        .completion(&uri, position_of(text, "let flag", 0), None)
        .await
        .expect("the script section completes");
    let CompletionResponse::Array(items) = response else {
        panic!("the server answers completion with a plain item array");
    };

    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert!(labels.contains(&"let"), "keywords are offered, got {labels:?}");
    assert!(
        labels.contains(&"var->"),
        "section markers are offered, got {labels:?}"
    );
}
