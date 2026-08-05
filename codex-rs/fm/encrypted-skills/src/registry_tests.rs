use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use super::*;

#[derive(Clone, Default)]
struct FakeClock(Arc<AtomicU64>);

impl FakeClock {
    fn new(start: u64) -> Self {
        Self(Arc::new(AtomicU64::new(start)))
    }

    fn advance(&self, millis: u64) {
        self.0.fetch_add(millis, Ordering::SeqCst);
    }
}

impl Clock for FakeClock {
    fn now_millis(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

fn registry_with(clock: Arc<FakeClock>) -> Registry {
    Registry::new(
        clock,
        TtlConfig {
            skill_idle: Duration::from_secs(60),
        },
    )
}

#[test]
fn register_then_loaded_within_ttl() {
    let clock = Arc::new(FakeClock::new(1000));
    let mut registry = registry_with(clock.clone());
    registry.register(
        "t1",
        "skill-a",
        PathBuf::from("/tmp/dir-a"),
        PathBuf::from("/orig"),
        "hex-token".to_string(),
    );
    clock.advance(30_000);
    assert!(registry.is_loaded("t1", "skill-a"));
}

#[test]
fn skill_ttl_expiry_makes_load_stale() {
    let clock = Arc::new(FakeClock::new(1000));
    let mut registry = registry_with(clock.clone());
    registry.register(
        "t1",
        "skill-a",
        PathBuf::from("/tmp/dir-a"),
        PathBuf::from("/orig"),
        "hex-token".to_string(),
    );
    clock.advance(61_000);
    assert!(!registry.is_loaded("t1", "skill-a"));
}

#[test]
fn touch_skill_refreshes_last_used_at() {
    let clock = Arc::new(FakeClock::new(1000));
    let mut registry = registry_with(clock.clone());
    registry.register(
        "t1",
        "skill-a",
        PathBuf::from("/tmp/dir-a"),
        PathBuf::from("/orig"),
        "hex-token".to_string(),
    );
    clock.advance(50_000);
    registry.touch_skill("t1", "skill-a");
    clock.advance(50_000);
    assert!(registry.is_loaded("t1", "skill-a"));
}

#[test]
fn sweep_expired_skills_evicts_only_expired() {
    let clock = Arc::new(FakeClock::new(1000));
    let mut registry = registry_with(clock.clone());
    registry.register(
        "t1",
        "expired",
        PathBuf::from("/tmp/expired"),
        PathBuf::from("/orig"),
        "hex-token".to_string(),
    );
    registry.register(
        "t1",
        "fresh",
        PathBuf::from("/tmp/fresh"),
        PathBuf::from("/orig"),
        "hex-token".to_string(),
    );
    clock.advance(61_000);
    registry.touch_skill("t1", "fresh");
    let evicted = registry.sweep_expired_skills();
    assert_eq!(evicted.len(), 1);
    assert_eq!(evicted[0].skill_name, "expired");
    assert_eq!(evicted[0].dir, PathBuf::from("/tmp/expired"));
    assert!(registry.is_loaded("t1", "fresh"));
    assert!(!registry.is_loaded("t1", "expired"));
}

#[test]
fn clear_thread_returns_dirs_and_removes_session() {
    let clock = Arc::new(FakeClock::new(1000));
    let mut registry = registry_with(clock);
    registry.register(
        "t1",
        "skill-a",
        PathBuf::from("/tmp/a"),
        PathBuf::from("/orig"),
        "hex-token".to_string(),
    );
    let dirs = registry.clear_thread("t1");
    assert_eq!(dirs, vec![PathBuf::from("/tmp/a")]);
    assert!(registry.get("t1", "skill-a").is_none());
    assert!(registry.clear_thread("t1").is_empty());
}

#[test]
fn register_returns_replaced_record() {
    let clock = Arc::new(FakeClock::new(1000));
    let mut registry = registry_with(clock);
    assert!(
        registry
            .register(
                "t1",
                "a",
                PathBuf::from("/tmp/a"),
                PathBuf::from("/orig"),
                "hex-token".to_string(),
            )
            .is_none()
    );
    let replaced = registry
        .register(
            "t1",
            "a",
            PathBuf::from("/tmp/a-new"),
            PathBuf::from("/orig"),
            "hex-token".to_string(),
        )
        .unwrap();
    assert_eq!(replaced.dir, PathBuf::from("/tmp/a"));
    assert_eq!(
        registry.get("t1", "a").map(|record| &record.dir),
        Some(&PathBuf::from("/tmp/a-new"))
    );
}

#[test]
fn clear_thread_only_removes_one_session() {
    let clock = Arc::new(FakeClock::new(1000));
    let mut registry = registry_with(clock);
    registry.register(
        "t1",
        "a",
        PathBuf::from("/tmp/a"),
        PathBuf::from("/orig"),
        "hex-token".to_string(),
    );
    registry.register(
        "t2",
        "b",
        PathBuf::from("/tmp/b"),
        PathBuf::from("/orig"),
        "hex-token".to_string(),
    );
    assert_eq!(registry.clear_thread("t1"), vec![PathBuf::from("/tmp/a")]);
    assert!(registry.get("t2", "b").is_some());
}
