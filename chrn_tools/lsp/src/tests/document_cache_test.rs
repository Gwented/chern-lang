use crate::state::{DocumentCache, DocumentState};
use crate::tests::session::{Session, TempWorkspace, position_of};
use chrn_utils::intern::Intern;
use std::sync::Arc;
use std::time::Duration;
use tokio::task::AbortHandle;
use tower_lsp::lsp_types::{Position, Range};

fn detached_state(text: Arc<String>, version: u64) -> DocumentState {
    DocumentState::new(
        text,
        Vec::new(),
        Vec::new(),
        Intern::init(),
        0,
        None,
        version,
    )
}

fn install_never_completing_task(session: &Session, uri: &str) -> AbortHandle {
    let task = tokio::spawn(std::future::pending());
    let abort = task.abort_handle();
    if let Some(previous) = session
        .backend()
        .pending_tasks
        .write()
        .insert(uri.to_string(), task)
    {
        previous.abort();
    }
    abort
}

#[test]
fn test_document_cache_lru() {
    let cache = DocumentCache::new(2);
    let uri1 = "file:///test1.chrn";
    let text1 = Arc::new("let x = 1".to_string());

    let state1 = cache.get_or_create(uri1, text1.clone(), 0, None, 1);
    assert_eq!(state1.read().version, 1);

    let state1_again = cache.get_or_create(uri1, text1.clone(), 0, None, 1);
    assert!(
        Arc::ptr_eq(&state1, &state1_again),
        "cache hit must return the same DocumentState Arc"
    );
    assert_eq!(
        state1_again.read().version,
        1,
        "cache hit must preserve the original version"
    );

    let uri2 = "file:///test2.chrn";
    let text2 = Arc::new("let y = 2".to_string());
    cache.get_or_create(uri2, text2, 0, None, 1);

    assert!(cache.get(uri1).is_some());

    let uri3 = "file:///test3.chrn";
    let text3 = Arc::new("let z = 3".to_string());
    cache.get_or_create(uri3, text3, 0, None, 1);

    assert!(cache.get(uri1).is_some(), "uri1 should be kept due to LRU");
    assert!(
        cache.get(uri2).is_none(),
        "uri2 should be evicted due to LRU"
    );
    assert!(cache.get(uri3).is_some(), "uri3 should be present");
}

#[test]
fn test_cache_invalidation() {
    let cache = DocumentCache::new(10);
    let uri_a = "file:///a.chrn";
    let uri_b = "file:///b.chrn";

    cache.get_or_create(uri_a, Arc::new("a".to_string()), 0, None, 1);
    cache.get_or_create(uri_b, Arc::new("b".to_string()), 0, None, 1);

    cache.register_dependencies(uri_a, &[uri_b.to_string()]);

    cache.invalidate(uri_b);

    assert!(cache.get(uri_b).is_none());
    assert!(cache.get(uri_a).is_none());
}

#[test]
fn test_document_cache_clear() {
    let cache = DocumentCache::new(10);
    cache.get_or_create("file:///a.chrn", Arc::new("a".to_string()), 0, None, 1);
    cache.get_or_create("file:///b.chrn", Arc::new("b".to_string()), 0, None, 1);

    cache.clear();

    assert!(
        cache.get("file:///a.chrn").is_none(),
        "clear must evict all entries"
    );
    assert!(cache.get("file:///b.chrn").is_none());
}

#[test]
fn test_document_cache_get_text() {
    let cache = DocumentCache::new(10);
    let uri = "file:///text_test.chrn";
    let text = Arc::new("let x = 42".to_string());

    cache.get_or_create(uri, Arc::clone(&text), 0, None, 1);

    let retrieved = cache
        .get_text(uri)
        .expect("get_text must return text for cached URI");
    assert_eq!(*retrieved, *text);
    assert!(cache.get_text("file:///missing.chrn").is_none());
}

#[test]
fn test_document_cache_for_each_state_visits_all() {
    let cache = DocumentCache::new(10);
    let uris = ["file:///p.chrn", "file:///q.chrn", "file:///r.chrn"];
    for u in &uris {
        cache.get_or_create(u, Arc::new("x".to_string()), 0, None, 1);
    }

    let mut visited = std::collections::HashSet::new();
    cache.for_each_state(|uri, _state| {
        visited.insert(uri.to_string());
    });

    for u in &uris {
        assert!(visited.contains(*u), "for_each_state must visit {}", u);
    }
}

#[test]
fn test_register_dependencies_re_registration_removes_old_edges() {
    let cache = DocumentCache::new(10);
    let uri_a = "file:///a.chrn";
    let uri_b = "file:///b.chrn";
    let uri_c = "file:///c.chrn";

    cache.get_or_create(uri_a, Arc::new("a".to_string()), 0, None, 1);
    cache.get_or_create(uri_b, Arc::new("b".to_string()), 0, None, 1);
    cache.get_or_create(uri_c, Arc::new("c".to_string()), 0, None, 1);

    cache.register_dependencies(uri_a, &[uri_b.to_string()]);
    cache.register_dependencies(uri_a, &[uri_c.to_string()]);

    cache.invalidate(uri_b);
    assert!(
        cache.get(uri_a).is_some(),
        "a should NOT be evicted after old dep b was re-registered away"
    );

    cache.invalidate(uri_c);
    assert!(
        cache.get(uri_a).is_none(),
        "a should be evicted because it now depends on c"
    );
}

#[test]
fn test_invalidating_an_importer_removes_its_reverse_dependency_edge() {
    let cache = DocumentCache::new(10);
    let uri_a = "file:///a.chrn";
    let uri_b = "file:///b.chrn";

    cache.get_or_create(uri_a, Arc::new("a".to_string()), 0, None, 1);
    cache.get_or_create(uri_b, Arc::new("b".to_string()), 0, None, 1);
    assert!(cache.register_dependencies(uri_a, &[uri_b.to_string()]));

    cache.invalidate(uri_a);
    assert_eq!(
        cache.dependency_graph_sizes(),
        (0, 0),
        "removing the final importer must remove both forward and empty reverse entries"
    );
    cache.get_or_create(uri_a, Arc::new("new a".to_string()), 0, None, 2);
    cache.invalidate(uri_b);

    assert!(
        cache.get(uri_a).is_some(),
        "the reinserted document has no dependency on b and must survive its invalidation"
    );
}

#[test]
fn test_evicting_an_importer_removes_its_reverse_dependency_edge() {
    let cache = DocumentCache::new(2);
    let uri_a = "file:///a.chrn";
    let uri_b = "file:///b.chrn";

    let state_a = cache.get_or_create(uri_a, Arc::new("a".to_string()), 0, None, 1);
    cache.get_or_create(uri_b, Arc::new("b".to_string()), 0, None, 1);
    assert!(cache.register_dependencies_for_state(uri_a, &state_a, &[uri_b.to_string()]));

    cache.get_or_create("file:///c.chrn", Arc::new("c".to_string()), 0, None, 1);

    assert!(cache.get(uri_a).is_none(), "a is the LRU entry");
    assert_eq!(
        cache.dependency_graph_sizes(),
        (0, 0),
        "eviction must remove both forward and empty reverse entries"
    );
}

#[test]
fn test_stale_state_cannot_restore_dependency_edges_after_replacement() {
    let cache = DocumentCache::new(10);
    let uri_a = "file:///a.chrn";
    let uri_b = "file:///b.chrn";

    let stale = cache.get_or_create(uri_a, Arc::new("old a".to_string()), 0, None, 1);
    cache.get_or_create(uri_b, Arc::new("b".to_string()), 0, None, 1);
    cache.invalidate(uri_a);
    let current = cache.get_or_create(uri_a, Arc::new("new a".to_string()), 0, None, 2);

    assert!(
        !cache.register_dependencies_for_state(uri_a, &stale, &[uri_b.to_string()]),
        "a completed stale analysis must not register dependencies for its replacement"
    );
    cache.invalidate(uri_b);

    let cached = cache
        .get(uri_a)
        .expect("rejecting the stale dependency keeps the replacement cached");
    assert!(
        Arc::ptr_eq(&cached, &current),
        "dependency invalidation must preserve the exact replacement state"
    );
}

#[test]
fn test_cache_miss_on_text_change() {
    let cache = DocumentCache::new(10);
    let uri = "file:///versioned.chrn";

    let text_v1 = Arc::new("let x = 1".to_string());
    let state_v1 = cache.get_or_create(uri, Arc::clone(&text_v1), 0, None, 1);

    cache.invalidate(uri);

    let text_v2 = Arc::new("let x = 2".to_string());
    let state_v2 = cache.get_or_create(uri, text_v2, 0, None, 2);

    assert!(
        !Arc::ptr_eq(&state_v1, &state_v2),
        "different text must yield a distinct DocumentState"
    );
    assert_eq!(state_v2.read().version, 2);
}

#[test]
fn test_guarded_insert_cannot_overwrite_current_state_when_gate_is_false() {
    let cache = DocumentCache::new(10);
    let uri = "file:///guarded.chrn";
    let current_text = Arc::new("let current = 1".to_string());
    let current = cache.get_or_create(uri, Arc::clone(&current_text), 0, None, 2);
    let stale_text = Arc::new("let stale = 1".to_string());

    let rejected = cache.insert_or_get_when(
        uri,
        Arc::clone(&stale_text),
        detached_state(Arc::clone(&stale_text), 1),
        || false,
    );

    assert!(rejected.is_none(), "a stale prepared state is rejected");
    let cached = cache.get(uri).expect("the current state remains cached");
    assert!(Arc::ptr_eq(&cached, &current));
    assert_eq!(cache.get_text(uri).as_deref(), Some(current_text.as_ref()));

    let accepted_text = Arc::new("let next = 1".to_string());
    let accepted = cache
        .insert_or_get_when(
            uri,
            Arc::clone(&accepted_text),
            detached_state(Arc::clone(&accepted_text), 3),
            || true,
        )
        .expect("a current prepared state is inserted");
    assert_eq!(accepted.read().version, 3);
    assert_eq!(cache.get_text(uri).as_deref(), Some(accepted_text.as_ref()));
}

#[test]
fn test_evicting_an_import_target_preserves_its_live_dependents() {
    let cache = DocumentCache::new(2);
    let uri_a = "file:///a.chrn";
    let uri_b = "file:///b.chrn";

    cache.get_or_create(uri_b, Arc::new("b".to_string()), 0, None, 1);
    let state_a = cache.get_or_create(uri_a, Arc::new("a".to_string()), 0, None, 1);
    assert!(cache.register_dependencies_for_state(uri_a, &state_a, &[uri_b.to_string()]));

    cache.get_or_create("file:///c.chrn", Arc::new("c".to_string()), 0, None, 1);
    assert!(cache.get(uri_b).is_none(), "b is the LRU entry");
    assert!(cache.get(uri_a).is_some(), "the importer remains cached");

    cache.invalidate(uri_b);
    assert!(
        cache.get(uri_a).is_none(),
        "invalidating an evicted target must still invalidate its cached importer"
    );
}

/// An incremental `did_change` must apply the edit to the stored text, invalidate the
/// cached analysis, and re-analyse the new text under a bumped version.
#[tokio::test(start_paused = true)]
async fn test_did_change_replaces_cached_text_and_bumps_the_version() {
    let workspace = TempWorkspace::new("did_change_recaches");
    let text = "let flag = 3\n";
    let uri = workspace.write("main.chrn", text);

    let mut session = Session::new().await;
    session.open(&uri, text).await;

    let opened_version = session
        .backend()
        .pending_versions
        .read()
        .get(uri.as_ref())
        .copied()
        .expect("did_open registers a version");

    let start = position_of(text, "flag", 0);
    let end = Position {
        line: start.line,
        character: start.character + 4,
    };
    session
        .change_range(&uri, Range { start, end }, "count")
        .await;

    assert_eq!(
        session
            .backend()
            .docs
            .read()
            .get(uri.as_ref())
            .map(|text| text.to_string())
            .as_deref(),
        Some("let count = 3\n"),
        "the ranged edit is applied to the stored document"
    );
    assert_eq!(
        session
            .backend()
            .doc_cache
            .get_text(uri.as_ref())
            .map(|text| text.to_string())
            .as_deref(),
        Some("let count = 3\n"),
        "re-analysis re-populates the cache with the edited text"
    );
    assert!(
        session
            .backend()
            .pending_versions
            .read()
            .get(uri.as_ref())
            .copied()
            .expect("the version entry survives the edit")
            > opened_version,
        "every change bumps the per-URI version counter"
    );
}

#[tokio::test(start_paused = true)]
async fn test_feature_reanalysis_registers_an_empty_dependency_set() {
    let workspace = TempWorkspace::new("empty_dependency_reregistration");
    let dependency_uri = workspace.write("dep.chrn", "export let ITEM = 1\n");
    let dependency_path = dependency_uri.to_file_path().unwrap();
    let imported_text = format!(
        "import \"{}\" as dep\nlet value = dep::ITEM\n",
        dependency_path.display()
    );
    let uri = workspace.write("main.chrn", &imported_text);
    let mut session = Session::new().await;
    session.open(&uri, &imported_text).await;
    assert_eq!(session.backend().doc_cache.dependency_graph_sizes(), (1, 1));

    let text_without_import = Arc::new("let value = 3\n".to_string());
    session
        .backend()
        .docs
        .write()
        .insert(uri.to_string(), Arc::clone(&text_without_import));
    assert!(
        session
            .hover(&uri, position_of(text_without_import.as_str(), "value", 0))
            .await
            .is_some(),
        "the feature request reanalyzes the replacement text"
    );
    assert_eq!(
        session.backend().doc_cache.dependency_graph_sizes(),
        (1, 0),
        "analysis with no imports replaces the old dependency set with an empty set"
    );

    session
        .backend()
        .doc_cache
        .invalidate(dependency_uri.as_ref());
    assert!(
        session.backend().doc_cache.get(uri.as_ref()).is_some(),
        "the reanalyzed document no longer depends on its former import"
    );
}

/// `did_close` must drop every per-document entry the backend holds. Reopening the
/// same URI must allocate a fresh process-wide generation so an old detached analysis
/// can never become current again through generation reuse.
#[tokio::test(start_paused = true)]
async fn test_did_close_clears_every_backend_entry() {
    let workspace = TempWorkspace::new("did_close_clears");
    let text = "let value = 3\n";
    let uri = workspace.write("main.chrn", text);

    let mut session = Session::new().await;
    session.open(&uri, text).await;
    let opened_generation = session.backend().pending_versions.read()[uri.as_ref()];
    assert!(
        session.backend().doc_cache.get(uri.as_ref()).is_some(),
        "the document is cached while open"
    );

    session.close(&uri).await;

    let backend = session.backend();
    assert!(
        !backend.docs.read().contains_key(uri.as_ref()),
        "the document text is dropped"
    );
    assert!(
        !backend.pending_versions.read().contains_key(uri.as_ref()),
        "the version counter is dropped"
    );
    assert!(
        !backend.diags_cache.read().contains_key(uri.as_ref()),
        "the diagnostic digest is dropped"
    );
    assert!(
        backend.doc_cache.get(uri.as_ref()).is_none(),
        "the analysis state is invalidated"
    );
    assert!(
        !backend.pending_tasks.read().contains_key(uri.as_ref()),
        "the tracked analysis task is removed"
    );

    session.open(&uri, "let reopened = 4\n").await;
    let reopened_generation = session.backend().pending_versions.read()[uri.as_ref()];
    assert!(
        reopened_generation > opened_generation,
        "reopening must not reuse generation {opened_generation}"
    );
    assert_eq!(
        session
            .backend()
            .doc_cache
            .get_text(uri.as_ref())
            .map(|text| text.to_string())
            .as_deref(),
        Some("let reopened = 4\n"),
        "the reopened document is analyzed from its new text"
    );
}

#[tokio::test(start_paused = true)]
async fn test_open_save_and_change_replace_the_tracked_analysis_task() {
    let workspace = TempWorkspace::new("analysis_task_replacement");
    let text = "let value = 1\n";
    let uri = workspace.write("main.chrn", text);

    let mut session = Session::new().await;
    let superseded_by_open = install_never_completing_task(&session, uri.as_ref());
    session.open(&uri, text).await;
    assert!(
        superseded_by_open.is_finished(),
        "didOpen aborts the previously tracked task"
    );
    let open_task = session.backend().pending_tasks.read()[uri.as_ref()].id();
    let open_generation = session.backend().pending_versions.read()[uri.as_ref()];

    let superseded_by_save = install_never_completing_task(&session, uri.as_ref());
    session.save(&uri, Some("let value = 2\n")).await;
    assert!(
        superseded_by_save.is_finished(),
        "didSave aborts the previously tracked task"
    );
    let save_task = session.backend().pending_tasks.read()[uri.as_ref()].id();
    let save_generation = session.backend().pending_versions.read()[uri.as_ref()];
    assert_ne!(
        save_task, open_task,
        "didSave replaces the tracked open task"
    );
    assert!(
        save_generation > open_generation,
        "didSave allocates a newer analysis generation"
    );

    let superseded_by_change = install_never_completing_task(&session, uri.as_ref());
    session.change_full(&uri, "let value = 3\n").await;
    assert!(
        superseded_by_change.is_finished(),
        "didChange aborts the previously tracked task"
    );
    let change_task = session.backend().pending_tasks.read()[uri.as_ref()].id();
    let change_generation = session.backend().pending_versions.read()[uri.as_ref()];
    assert_ne!(
        change_task, save_task,
        "didChange replaces the tracked save task"
    );
    assert!(
        change_generation > save_generation,
        "didChange allocates a newer analysis generation"
    );
    assert_eq!(
        session.backend().pending_tasks.read().len(),
        1,
        "only the latest task remains tracked for the open URI"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_hover_times_out_on_state_lock_contention_and_server_recovers() {
    let workspace = TempWorkspace::new("hover_lock_contention");
    let text = "let value = 3\n";
    let uri = workspace.write("main.chrn", text);
    let mut session = Session::new().await;
    session.open(&uri, text).await;

    let state = session
        .backend()
        .doc_cache
        .get(uri.as_ref())
        .expect("the open document has cached analysis");
    let (locked_tx, locked_rx) = std::sync::mpsc::sync_channel(0);
    let (release_tx, release_rx) = std::sync::mpsc::sync_channel(0);
    let lock_thread = std::thread::spawn(move || {
        let _guard = state.write();
        locked_tx.send(()).unwrap();
        release_rx.recv().unwrap();
    });
    locked_rx.recv().unwrap();

    let request_uri = uri.clone();
    let request = tokio::spawn(async move {
        let hover = session
            .hover(&request_uri, position_of(text, "value", 0))
            .await;
        (session, hover)
    });
    let blocked_result = tokio::time::timeout(Duration::from_secs(2), request).await;

    // Always release the writer before interpreting a timeout so a regression
    // fails cleanly instead of leaking the lock thread and hanging the test process.
    release_tx.send(()).unwrap();
    lock_thread.join().unwrap();
    let (mut session, blocked_hover) = blocked_result
        .expect("the feature lock budget bounds the response")
        .expect("the request task completes");
    assert!(
        blocked_hover.is_none(),
        "contention returns no hover result"
    );
    let recovered_hover = tokio::time::timeout(
        Duration::from_secs(2),
        session.hover(&uri, position_of(text, "value", 0)),
    )
    .await
    .expect("the server remains responsive after contention");
    assert!(
        recovered_hover.is_some(),
        "hover succeeds after the lock is released"
    );
}
