use crate::state::DocumentCache;
use std::sync::Arc;

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
