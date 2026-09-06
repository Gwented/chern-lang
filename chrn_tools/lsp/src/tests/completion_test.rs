use crate::backend::{ConfigCompletionCandidate, config_completion_items};
use crate::state::{DocumentState, SemanticEntity};
use crate::tests::session::{Session, TempWorkspace, position_of};
use chrn_utils::id_types::{InternedId, SourceRegionId};
use chrn_utils::intern::Intern;
use chrn_utils::source_map::source_span::SourceSpan;
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
        scope_id: None,
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

/// Completing through the current module's namespace exposes symbols owned by that
/// module, but not built-in types injected from core. Keeping a same-prefix user type
/// in the fixture proves the prefix itself is not being rejected.
#[tokio::test(start_paused = true)]
async fn current_module_completion_keeps_local_types_and_hides_injected_core_types() {
    let workspace = TempWorkspace::new("module_scope_completion");
    let text = "nest->\nstruct item { value: i32 }\nmain::i\n";
    let uri = workspace.write("main.chrn", text);

    let mut session = Session::new().await;
    session.open(&uri, text).await;

    // Cursor directly after the typed prefix so the `::` trigger applies.
    let mut pos = position_of(text, "main::i", 0);
    pos.character += "main::i".len() as u32;

    let response = session
        .completion(&uri, pos, None)
        .await
        .expect("the script section completes");
    let CompletionResponse::Array(items) = response else {
        panic!("the server answers completion with a plain item array");
    };

    let item = items
        .iter()
        .find(|completion| completion.label == "item")
        .unwrap_or_else(|| panic!("`main::i` retains the local type, got {items:?}"));
    assert_eq!(
        item.kind,
        Some(tower_lsp::lsp_types::CompletionItemKind::STRUCT),
        "the local declaration remains a struct completion"
    );
    assert!(
        items.iter().all(|completion| completion.label != "i8"),
        "the core-injected `i8` type is not owned by `main`, got {items:?}"
    );
}

/// Override paths use compiler-provided namespace scopes rather than module
/// exports. Completing a partially typed `java::int` must still expose the
/// terminal extern type.
#[tokio::test(start_paused = true)]
async fn static_access_on_an_intrinsic_namespace_offers_extern_types() {
    let workspace = TempWorkspace::new("intrinsic_static_completion");
    let text = "complex->\n    override JAVA {\n        types {\n            change i8 = java::i\n        }\n    }\n";
    let uri = workspace.write("main.chrn", text);

    let mut session = Session::new().await;
    session.open(&uri, text).await;

    let mut pos = position_of(text, "java::i", 0);
    pos.character += "java::i".len() as u32;

    let response = session
        .completion(&uri, pos, None)
        .await
        .expect("the override path completes");
    let CompletionResponse::Array(items) = response else {
        panic!("the server answers completion with a plain item array");
    };

    let labels: Vec<&str> = items.iter().map(|item| item.label.as_str()).collect();
    let int = items
        .iter()
        .find(|item| item.label == "int")
        .unwrap_or_else(|| panic!("`java::i` completes the extern type `int`, got {labels:?}"));
    assert_eq!(
        int.kind,
        Some(tower_lsp::lsp_types::CompletionItemKind::CLASS)
    );
}

/// A repeated intrinsic segment name such as `types` must be resolved through
/// the preceding root namespace. Looking it up globally can select the sibling
/// platform's `types` scope and return the wrong language namespace.
#[tokio::test(start_paused = true)]
async fn multi_segment_intrinsic_completion_stays_under_its_root_namespace() {
    use tower_lsp::lsp_types::CompletionItemKind;

    let workspace = TempWorkspace::new("qualified_intrinsic_static_completion");
    let mut session = Session::new().await;

    for (root, expected) in [("JAVA", "java"), ("RUST", "rust")] {
        let qualified = format!("{root}::types::");
        let text = format!(
            "complex->\n    override {root} {{\n        types {{\n            change i8 = {qualified}\n        }}\n    }}\n"
        );
        let uri = workspace.write(&format!("{}.chrn", root.to_lowercase()), &text);
        session.open(&uri, &text).await;

        let mut pos = position_of(&text, &qualified, 0);
        pos.character += qualified.len() as u32;
        let response = session
            .completion(&uri, pos, None)
            .await
            .unwrap_or_else(|| panic!("`{qualified}` completes"));
        let CompletionResponse::Array(items) = response else {
            panic!("completion must return an item array");
        };

        let mut actual: Vec<_> = items
            .into_iter()
            .map(|item| (item.label, item.kind))
            .collect();
        actual.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            actual,
            vec![(expected.to_string(), Some(CompletionItemKind::VARIABLE))],
            "`{qualified}` completes only its own language namespace"
        );
    }
}

/// An embedded override introduces its own intrinsic namespace. Completion
/// immediately after its shorthand arrow must offer that namespace's children
/// even when the incomplete document contains an invalid shorthand child after
/// the cursor. Losing the override HIR must not fall back to the enclosing config.
#[tokio::test(start_paused = true)]
async fn embedded_override_arrow_completes_the_intrinsic_namespace() {
    use tower_lsp::lsp_types::CompletionItemKind;

    let workspace = TempWorkspace::new("embedded_override_arrow_completion");
    let text = "nest->\nstruct Structure { field1: i32, field2: u32 }\ncomplex->\nfor Structure {\n    field1 {\n        override JAVA=>idents { change i32 = rust::char }\n    }\n}\n";
    let uri = workspace.write("main.chrn", text);

    let mut session = Session::new().await;
    session.open(&uri, text).await;

    let mut position = position_of(text, "override JAVA=>", 0);
    position.character += "override JAVA=>".len() as u32;
    let response = session
        .completion(&uri, position, Some(">"))
        .await
        .expect("the embedded override completes");
    let CompletionResponse::Array(items) = response else {
        panic!("completion must return an item array");
    };
    let mut actual: Vec<_> = items
        .iter()
        .map(|item| (item.label.as_str(), item.kind))
        .collect();
    actual.sort_by_key(|(label, _)| *label);
    assert_eq!(
        actual,
        vec![("types", Some(CompletionItemKind::VARIABLE))],
        "`JAVA=>` exposes only its namespace child despite invalid shorthand content"
    );
    {
        let state = session.backend().doc_cache.get(uri.as_str()).unwrap();
        let state = state.read();
        let java_start = text.find("JAVA").unwrap() - state.script_start;
        let java_span = SourceSpan::new(
            SourceRegionId::new(0),
            java_start as u32,
            (java_start + "JAVA".len()) as u32,
        );
        let exact_entities: Vec<_> = state
            .symbol_map
            .iter()
            .filter_map(|(span, entity)| (span == &java_span).then_some(entity))
            .collect();
        assert_eq!(
            exact_entities.len(),
            1,
            "the shorthand override name has one semantic identity, got {exact_entities:?}"
        );
        let entity = exact_entities[0];
        let SemanticEntity::Symbol(sym_id) = entity else {
            panic!("the shorthand override namespace is a symbol, not a config member: {entity:?}");
        };
        assert!(
            matches!(
                state.compiler.as_ref().unwrap().syms[*sym_id].kind,
                compilation::semantic::hir::hir_symbols::SymbolKind::Namespace
            ),
            "the duplicate-span lookup selects the intrinsic namespace symbol"
        );
    }
}

/// `override` selects an intrinsic configuration root rather than an ordinary
/// type-based config target. Completion at that grammar position must expose
/// exactly the compiler's available platform namespaces.
#[tokio::test(start_paused = true)]
async fn override_root_completion_offers_intrinsic_namespaces() {
    use tower_lsp::lsp_types::CompletionItemKind;

    let workspace = TempWorkspace::new("override_root_completion");
    let mut session = Session::new().await;

    for (name, prefix, expected) in [
        ("empty", "", &["JAVA", "RUST"][..]),
        ("java_prefix", "J", &["JAVA"][..]),
        ("rust_prefix", "RU", &["RUST"][..]),
    ] {
        let target = format!("override {prefix}");
        let text = format!("complex->\n{target}\n");
        let uri = workspace.write(&format!("{name}.chrn"), &text);
        session.open(&uri, &text).await;

        let mut position = position_of(&text, &target, 0);
        position.character += target.len() as u32;
        let response = session
            .completion(&uri, position, None)
            .await
            .unwrap_or_else(|| panic!("the `{target}` target completes"));
        let CompletionResponse::Array(items) = response else {
            panic!("completion must return an item array");
        };

        let mut actual: Vec<_> = items
            .into_iter()
            .map(|item| (item.label, item.kind))
            .collect();
        actual.sort_by(|a, b| a.0.cmp(&b.0));
        let expected: Vec<_> = expected
            .iter()
            .map(|label| (label.to_string(), Some(CompletionItemKind::VARIABLE)))
            .collect();
        assert_eq!(
            actual, expected,
            "`{target}` offers only matching intrinsic platform namespaces"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn arrow_config_completion_matches_braces_for_struct_members() {
    use tower_lsp::lsp_types::CompletionItemKind;

    let workspace = TempWorkspace::new("arrow_config_completion");
    let mut session = Session::new().await;
    let declarations = "nest->\nstruct Inner { available: i32 }\nstruct Outer { inner: Inner, unrelated: i32 }\ncomplex->\n";

    for (name, config, trigger) in [
        ("braces", "for Outer { inner {~\n} }\n", None),
        ("arrow", "for Outer { inner =>~\n} \n", Some(">")),
        (
            "arrow_whitespace",
            "for Outer { inner =>\n    ~\n} \n",
            None,
        ),
    ] {
        let marked = format!("{declarations}{config}");
        let position = position_of(&marked, "~", 0);
        let text = marked.replace('~', "");
        let uri = workspace.write(&format!("{name}.chrn"), &text);
        let diagnostics = session.open(&uri, &text).await;
        assert!(diagnostics.is_empty(), "{name}: {diagnostics:?}");
        assert_eq!(
            session
                .backend()
                .docs
                .read()
                .get(uri.as_str())
                .unwrap()
                .as_str(),
            text
        );

        let response = session.completion(&uri, position, trigger).await.unwrap();
        let CompletionResponse::Array(items) = response else {
            panic!("{name}: completion must return an item array");
        };
        let mut actual: Vec<_> = items
            .into_iter()
            .map(|item| (item.label, item.kind))
            .collect();
        actual.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            actual,
            vec![
                ("available".into(), Some(CompletionItemKind::FIELD)),
                ("cases".into(), Some(CompletionItemKind::PROPERTY)),
                ("default_val".into(), Some(CompletionItemKind::PROPERTY)),
                ("idents".into(), Some(CompletionItemKind::PROPERTY)),
            ],
            "{name}: complete the inner member using its type and the member option schema"
        );
    }
}

#[tokio::test(start_paused = true)]
async fn scalar_arrow_completion_uses_member_options_and_stops_at_parent_close() {
    use tower_lsp::lsp_types::CompletionItemKind;

    let workspace = TempWorkspace::new("scalar_arrow_completion");
    let text =
        "nest->\nstruct Outer { value: i32, sibling: i32 }\ncomplex->\nfor Outer { value =>\n}\n\n";
    let uri = workspace.write("main.chrn", text);
    let mut session = Session::new().await;
    let diagnostics = session.open(&uri, text).await;
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        session
            .backend()
            .docs
            .read()
            .get(uri.as_str())
            .unwrap()
            .as_str(),
        text
    );

    let mut position = position_of(text, "value =>", 0);
    position.character += "value =>".len() as u32;
    let response = session.completion(&uri, position, Some(">")).await.unwrap();
    let CompletionResponse::Array(items) = response else {
        panic!("completion must return an item array");
    };
    let mut actual: Vec<_> = items
        .into_iter()
        .map(|item| (item.label, item.kind))
        .collect();
    actual.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        actual,
        vec![
            ("cases".into(), Some(CompletionItemKind::PROPERTY)),
            ("default_val".into(), Some(CompletionItemKind::PROPERTY)),
            ("idents".into(), Some(CompletionItemKind::PROPERTY)),
        ]
    );

    let mut after_close = position_of(text, "}\n\n", 0);
    after_close.line += 1;
    after_close.character = 0;
    let response = session.completion(&uri, after_close, None).await.unwrap();
    let CompletionResponse::Array(items) = response else {
        panic!("completion must return an item array");
    };
    assert!(items.iter().any(|item| item.label == "let"));
}
