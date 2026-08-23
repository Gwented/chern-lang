use crate::state::DocumentCache;
use crate::tests::session::{Session, TempWorkspace, position_of};
use std::sync::Arc;
use tower_lsp::lsp_types::{Position, Range};

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

/// `did_close` must drop every per-document entry the backend holds, so a reopened
/// document cannot observe stale text, versions, or diagnostic digests.
#[tokio::test(start_paused = true)]
async fn test_did_close_clears_every_backend_entry() {
    let workspace = TempWorkspace::new("did_close_clears");
    let text = "let value = 3\n";
    let uri = workspace.write("main.chrn", text);

    let mut session = Session::new().await;
    session.open(&uri, text).await;
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
}
