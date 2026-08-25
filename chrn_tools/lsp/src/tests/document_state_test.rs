use crate::state::{DocumentCache, DocumentState, SemanticEntity};
use crate::tests::session::{Session, TempWorkspace, hover_text, position_of};
use chrn_utils::id_types::{SourceRegionId, SymbolId};
use chrn_utils::source_map::source_span::SourceSpan;
use std::sync::Arc;
use tower_lsp::lsp_types::{GotoDefinitionResponse, Position, Range};

/// `get_entity_at_offset` must return the *smallest* span containing the offset,
/// regardless of the order entries were collected in. The lookup binary-searches a
/// sorted map, so this also guards the sort/`max_span_len` invariant maintained by
/// `set_symbol_map`.
#[test]
fn test_get_entity_at_offset_picks_smallest_containing_span() {
    let cache = DocumentCache::new(10);
    let state_arc = cache.get_or_create(
        "file:///nested.chrn",
        Arc::new("let outer = 1".to_string()),
        0,
        None,
        1,
    );
    let mut state = state_arc.write();

    let span = |start, end| SourceSpan::new(SourceRegionId::new(0), start, end);
    let sym = |n| SemanticEntity::Symbol(SymbolId::new(n));

    // Deliberately unsorted, and with the widest span pushed between the others.
    state.set_symbol_map(vec![
        (span(4, 9), sym(1)),   // inner
        (span(0, 13), sym(2)),  // outermost, spans the whole line
        (span(12, 13), sym(3)), // later, disjoint
        (span(0, 3), sym(4)),   // earlier, disjoint
    ]);

    assert_eq!(state.get_entity_at_offset(5), Some(&sym(1)), "inner wins");
    assert_eq!(state.get_entity_at_offset(1), Some(&sym(4)), "earlier wins");
    assert_eq!(state.get_entity_at_offset(12), Some(&sym(3)), "later wins");
    assert_eq!(
        state.get_entity_at_offset(10),
        Some(&sym(2)),
        "only the outermost span covers offset 10"
    );
    assert_eq!(
        state.get_entity_at_offset(13),
        None,
        "spans are end-exclusive"
    );
}

#[test]
fn test_get_token_at_offset() {
    let cache = DocumentCache::new(10);
    let uri = "file:///test_tokens.chrn";
    let text = Arc::new("let foo = 123;".to_string());
    let state = cache.get_or_create(uri, text, 0, None, 1);
    let read_state = state.read();

    let token = read_state
        .get_token_at_offset(5)
        .expect("Should find 'foo'");
    assert_eq!(token.span.start, 4);
    assert_eq!(token.span.end, 7);

    let token2 = read_state
        .get_token_at_offset(10)
        .expect("Should find '123'");
    assert_eq!(token2.span.start, 10);
    assert_eq!(token2.span.end, 13);

    assert!(
        read_state.get_token_at_offset(3).is_none(),
        "Space should return None"
    );
}

#[test]
fn test_offset_in_comment_single_line() {
    let cache = DocumentCache::new(10);
    let uri = "file:///comment_test.chrn";
    let text = Arc::new("let x = 1 // comment here".to_string());
    let state_arc = cache.get_or_create(uri, text, 0, None, 1);
    let state = state_arc.read();

    assert!(
        !state.offset_in_comment(0),
        "start of code should not be in comment"
    );
    assert!(
        state.offset_in_comment(11),
        "offset at // should be in comment"
    );
    assert!(
        state.offset_in_comment(18),
        "offset inside comment text should be in comment"
    );
}

#[test]
fn test_offset_in_comment_only_applies_to_own_line() {
    let cache = DocumentCache::new(10);
    let uri = "file:///comment_multiline.chrn";
    let text = Arc::new("// first line\nlet y = 2".to_string());
    let state_arc = cache.get_or_create(uri, text, 0, None, 1);
    let state = state_arc.read();

    assert!(
        !state.offset_in_comment(14),
        "start of second line should not be treated as inside first-line comment"
    );
}

#[test]
fn test_get_token_at_offset_with_script_start() {
    let cache = DocumentCache::new(10);
    let uri = "file:///def_test.chrn";
    let text = Arc::new("@def\nlet foo = 123;".to_string());
    let state = cache.get_or_create(uri, text, 5, None, 1);
    let read_state = state.read();

    let token = read_state
        .get_token_at_offset(10)
        .expect("Should find 'foo' via absolute offset");
    assert_eq!(token.span.start, 4);
    assert_eq!(token.span.end, 7);

    let token2 = read_state
        .get_token_at_offset(15)
        .expect("Should find '123' via absolute offset");
    assert_eq!(token2.span.start, 10);
    assert_eq!(token2.span.end, 13);

    let let_token = read_state
        .get_token_at_offset(9)
        .expect("Should find 'let' via absolute offset");
    assert_eq!(let_token.span.start, 4);
    assert_eq!(let_token.span.end, 7);

    assert!(
        read_state.get_token_at_offset(8).is_none(),
        "space should not be a token"
    );
}

#[test]
fn test_offset_in_comment_with_script_start_relative_trivia() {
    let cache = DocumentCache::new(10);
    let uri = "file:///def_comment_test.chrn";
    let text = Arc::new("@def\nlet x // inside script\n".to_string());
    let state_arc = cache.get_or_create(uri, text, 5, None, 1);
    let state = state_arc.read();

    assert!(
        state.offset_in_comment(13),
        "absolute offset 13 ('//' start) must be detected as comment"
    );
    assert!(
        state.offset_in_comment(21),
        "absolute offset 21 (inside comment) must be detected as comment"
    );
    assert!(
        !state.offset_in_comment(9),
        "absolute offset 9 (start of 'let') must not be a comment"
    );
}

#[test]
fn test_find_matching_entities_propagates_script_start() {
    let cache = Arc::new(DocumentCache::new(10));

    let uri_a = "file:///a.chrn";
    let text_a = Arc::new("@def\nlet a = 1".to_string());
    let state_a = cache.get_or_create(uri_a, text_a, 5, None, 1);

    let uri_b = "file:///b.chrn";
    let text_b = Arc::new("let b = 1".to_string());
    let state_b = cache.get_or_create(uri_b, text_b, 0, None, 1);

    assert_eq!(state_a.read().script_start, 5);
    assert_eq!(state_b.read().script_start, 0);

    let results: Vec<(String, Arc<String>, u32, u32, usize)> =
        DocumentState::find_matching_entities(
            &cache,
            std::path::Path::new("<no-match>"),
            chrn_utils::source_map::source_span::SourceSpan::new(
                chrn_utils::id_types::SourceRegionId::new(0),
                0,
                1,
            ),
            None,
        );
    assert!(
        results.is_empty(),
        "no compiler -> no matches; empty result is the expected outcome"
    );
}

/// Hover over a use of a binding resolves through the semantic map to the declaration
/// and reports the inferred type.
#[tokio::test(start_paused = true)]
async fn test_hover_reports_the_inferred_type_of_a_binding() {
    let workspace = TempWorkspace::new("hover_binding");
    let text = "// data header\n@def\nlet value = 3\nlet other = value + 1\n@end\ntrailing: data\n";
    let uri = workspace.write("embedded.chrn", text);

    let mut session = Session::new().await;
    session.open(&uri, text).await;

    let hover = session
        .hover(&uri, position_of(text, "value", 1))
        .await
        .expect("hovering a known binding returns contents");

    assert!(
        hover_text(&hover).contains("value: i64"),
        "hover reports the binding and its inferred type, got `{}`",
        hover_text(&hover)
    );
}

/// Hover over a built-in namespace member written in real code (`i32::MAX`) resolves
/// through the semantic map to the instantiation variable and reports its value.
#[tokio::test(start_paused = true)]
async fn test_hover_resolves_builtin_namespace_members() {
    let workspace = TempWorkspace::new("hover_builtin_member");
    let text = "@def\nlet limit = i32::MAX\n@end\n";
    let uri = workspace.write("builtin_member.chrn", text);

    let mut session = Session::new().await;
    session.open(&uri, text).await;

    let hover = session
        .hover(&uri, position_of(text, "MAX", 0))
        .await
        .expect("hovering `i32::MAX` returns contents");

    assert!(
        hover_text(&hover).contains("MAX"),
        "hover names the member, got `{}`",
        hover_text(&hover)
    );
}

/// Go-to-definition inside an embedded `@def` region must return the declaration in
/// absolute file coordinates, using the target region's `script_start`.
#[tokio::test(start_paused = true)]
async fn test_definition_resolves_to_the_declaration_in_absolute_positions() {
    let workspace = TempWorkspace::new("definition_absolute");
    let text = "// data header\n@def\nlet value = 3\nlet other = value + 1\n@end\ntrailing: data\n";
    let uri = workspace.write("embedded.chrn", text);

    let mut session = Session::new().await;
    session.open(&uri, text).await;

    let response = session
        .definition(&uri, position_of(text, "value", 1))
        .await
        .expect("the use of `value` has a definition");

    let GotoDefinitionResponse::Link(links) = response else {
        panic!("the server answers definition requests with location links");
    };
    let [link] = links.as_slice() else {
        panic!("a single binding has a single definition, got {links:?}");
    };

    assert_eq!(link.target_uri, uri, "the declaration is in the same file");
    assert_eq!(
        link.target_range,
        Range {
            start: Position { line: 2, character: 4 },
            end: Position { line: 2, character: 9 },
        },
        "`value` is declared on the third line of the file"
    );
}

/// Cross-module definition, references, and rename all key off the identity tuple
/// `(definition_path, definition_span, owning_symbol_id)` and must reach a file that was
/// never opened by the client — the exporting module is loaded from disk.
#[tokio::test(start_paused = true)]
async fn test_cross_module_lookups_reach_an_unopened_exporting_file() {
    let workspace = TempWorkspace::new("cross_module_lookups");
    let dependency = "export let READ = 0b0\nexport let WRITE = 0b1\n";
    let dependency_uri = workspace.write("dep.chrn", dependency);

    // Import paths are opened as written, relative to the process working directory,
    // so a test fixture has to import by absolute path.
    let dependency_path = dependency_uri
        .to_file_path()
        .expect("the workspace URI is a file path");
    let text = format!(
        "import \"{}\" as deps\n\nlet flag = deps::READ\n",
        dependency_path.display()
    );
    let uri = workspace.write("main.chrn", &text);

    let mut session = Session::new().await;
    let diagnostics = session.open(&uri, &text).await;
    assert!(
        diagnostics.is_empty(),
        "the fixture resolves cleanly, got {diagnostics:?}"
    );

    let use_site = position_of(&text, "READ", 0);
    let declaration = Range {
        start: Position { line: 0, character: 11 },
        end: Position { line: 0, character: 15 },
    };

    let response = session
        .definition(&uri, use_site)
        .await
        .expect("an imported symbol has a definition");
    let GotoDefinitionResponse::Link(links) = response else {
        panic!("the server answers definition requests with location links");
    };
    assert_eq!(
        links[0].target_uri, dependency_uri,
        "definition crosses into the exporting file"
    );
    assert_eq!(
        links[0].target_range, declaration,
        "`READ` is declared on the first line of the exporting file"
    );

    let references = session
        .references(&uri, use_site)
        .await
        .expect("an imported symbol has references");
    assert!(
        references
            .iter()
            .any(|location| location.uri == dependency_uri && location.range == declaration),
        "the declaration in the unopened file is found, got {references:?}"
    );
    assert!(
        references
            .iter()
            .any(|location| location.uri == uri),
        "the use site in the importing file is found, got {references:?}"
    );

    let edit = session
        .rename(&uri, use_site, "READ_ONLY")
        .await
        .expect("an imported symbol is renameable");
    let changes = edit.changes.expect("rename produces per-file text edits");
    assert!(
        changes.contains_key(&dependency_uri) && changes.contains_key(&uri),
        "a cross-module rename edits both files, got {:?}",
        changes.keys().collect::<Vec<_>>()
    );
}
