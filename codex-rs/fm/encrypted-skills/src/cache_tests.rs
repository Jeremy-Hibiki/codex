use super::*;

fn content(plaintext: &str) -> CachedContent {
    CachedContent {
        plaintext: Zeroizing::new(plaintext.to_string()),
        skill_name: "skill".to_string(),
        base_dir: None,
    }
}

#[test]
fn store_returns_distinct_tokens_for_distinct_plaintext() {
    let mut cache = ContentCache::new(64);
    let a = cache.store("s1", content("alpha"), |_| false);
    let b = cache.store("s1", content("beta"), |_| false);
    assert_ne!(a, b);
    assert_eq!(
        cache.lookup("s1", &a).map(|c| c.plaintext.as_str()),
        Some("alpha")
    );
    assert_eq!(
        cache.lookup("s1", &b).map(|c| c.plaintext.as_str()),
        Some("beta")
    );
}

#[test]
fn store_deduplicates_identical_plaintext() {
    let mut cache = ContentCache::new(64);
    let a = cache.store("s1", content("same"), |_| false);
    let b = cache.store("s1", content("same"), |_| false);
    assert_eq!(a, b);
}

#[test]
fn sessions_are_isolated() {
    let mut cache = ContentCache::new(64);
    let a = cache.store("s1", content("text"), |_| false);
    assert_eq!(
        cache.lookup("s1", &a).map(|c| c.plaintext.as_str()),
        Some("text")
    );
    assert_eq!(cache.lookup("s2", &a), None);
}

#[test]
fn fifo_eviction_respects_cap() {
    let mut cache = ContentCache::new(2);
    let a = cache.store("s1", content("one"), |_| false);
    let b = cache.store("s1", content("two"), |_| false);
    let c = cache.store("s1", content("three"), |_| false);
    assert_eq!(cache.lookup("s1", &a), None);
    assert_eq!(
        cache.lookup("s1", &b).map(|c| c.plaintext.as_str()),
        Some("two")
    );
    assert_eq!(
        cache.lookup("s1", &c).map(|c| c.plaintext.as_str()),
        Some("three")
    );
}

#[test]
fn drop_session_removes_all_entries() {
    let mut cache = ContentCache::new(64);
    let a = cache.store("s1", content("one"), |_| false);
    cache.drop_session("s1");
    assert_eq!(cache.lookup("s1", &a), None);
}

#[test]
fn plaintexts_iterates_session_entries() {
    let mut cache = ContentCache::new(64);
    cache.store("s1", content("one"), |_| false);
    cache.store("s1", content("two"), |_| false);
    cache.store("s2", content("other"), |_| false);
    let mut values: Vec<&str> = cache.plaintexts("s1").collect();
    values.sort();
    assert_eq!(values, vec!["one", "two"]);
}

#[test]
fn total_size_cap_evicts_oldest() {
    let mut cache = ContentCache::new_with_limits(64, 10);
    let a = cache.store("s1", content("aaaaaa"), |_| false); // 6 bytes
    let b = cache.store("s1", content("bbbbbb"), |_| false); // 12 bytes total -> evicts a
    let c = cache.store("s1", content("cccccc"), |_| false); // 12 bytes total -> evicts b
    assert_eq!(cache.lookup("s1", &a), None);
    assert_eq!(cache.lookup("s1", &b), None);
    assert_eq!(
        cache.lookup("s1", &c).map(|c| c.plaintext.as_str()),
        Some("cccccc")
    );
}

#[test]
fn referenced_entries_survive_cap_eviction() {
    let mut cache = ContentCache::new(1);
    let a = cache.store("s1", content("one"), |_| false);
    let b = cache.store("s1", content("two"), |hex| hex == a);
    assert_eq!(
        cache.lookup("s1", &a).map(|c| c.plaintext.as_str()),
        Some("one"),
        "referenced entry must not be evicted"
    );
    assert_eq!(
        cache.lookup("s1", &b).map(|c| c.plaintext.as_str()),
        Some("two")
    );
}
