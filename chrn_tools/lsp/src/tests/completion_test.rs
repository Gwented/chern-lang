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
    assert!(
        labels.contains(&"let"),
        "keywords are offered, got {labels:?}"
    );
    assert!(
        labels.contains(&"var->"),
        "section markers are offered, got {labels:?}"
    );
}

/// Static access on a built-in type offers its namespace members (`MAX`, `MIN`),
/// which live in builtin-type namespace scopes rather than any module.
#[tokio::test(start_paused = true)]
async fn static_access_on_a_builtin_type_offers_its_namespace_members() {
    let workspace = TempWorkspace::new("builtin_static_completion");
    let text = "let flag = 3\ni32::M\n";
    let uri = workspace.write("main.chrn", text);

    let mut session = Session::new().await;
    session.open(&uri, text).await;

    // Cursor directly after the typed prefix so the `::` trigger applies.
    let mut pos = position_of(text, "M", 0);
    pos.character += 1;

    let response = session
        .completion(&uri, pos, None)
        .await
        .expect("the script section completes");
    let CompletionResponse::Array(items) = response else {
        panic!("the server answers completion with a plain item array");
    };

    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    assert!(
        labels.contains(&"MAX"),
        "`i32::` completes MAX, got {labels:?}"
    );
    assert!(
        labels.contains(&"MIN"),
        "`i32::` completes MIN, got {labels:?}"
    );
}

/// Completing inside the current module's namespace never offers compiler-internal
/// namespace members such as `i8::MAX`; they are unreachable through scope lookup.
#[tokio::test(start_paused = true)]
async fn current_module_completion_hides_builtin_namespace_members() {
    let workspace = TempWorkspace::new("module_scope_completion");
    let text = "let flag = 3\nmain::M\n";
    let uri = workspace.write("main.chrn", text);

    let mut session = Session::new().await;
    session.open(&uri, text).await;

    // Cursor directly after the typed prefix so the `::` trigger applies.
    let mut pos = position_of(text, "main::M", 0);
    pos.character += "main::M".len() as u32;

    let response = session
        .completion(&uri, pos, None)
        .await
        .expect("the script section completes");
    let CompletionResponse::Array(items) = response else {
        panic!("the server answers completion with a plain item array");
    };

    let max_count = items.iter().filter(|item| item.label == "MAX").count();
    let min_count = items.iter().filter(|item| item.label == "MIN").count();
    assert_eq!(
        max_count, 0,
        "MAX is not reachable from a module, got {max_count} items"
    );
    assert_eq!(
        min_count, 0,
        "MIN is not reachable from a module, got {min_count} items"
    );
}
