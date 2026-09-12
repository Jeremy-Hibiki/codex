use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::Duration;

use fm_encrypted_skills::registry::Clock;
use fm_encrypted_skills::registry::TtlConfig;
use fm_encrypted_skills::runtime::EncryptedSkillRuntime;
use fm_encrypted_skills::sdk::EnvelopeError;
use fm_encrypted_skills::sdk::EnvelopeSdk;
use fm_encrypted_skills::sdk::PackageEntry;

use super::spawn_periodic_sweep;

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

struct PeriodicSweepSdk;

impl EnvelopeSdk for PeriodicSweepSdk {
    fn decrypt_package(&self, _package_path: &Path) -> Result<Vec<PackageEntry>, EnvelopeError> {
        Ok(vec![PackageEntry {
            rel_path: PathBuf::from("SKILL.md"),
            contents: b"# periodic skill".to_vec(),
        }])
    }
}

#[tokio::test(start_paused = true)]
async fn periodic_sweep_unloads_idle_skill_without_requests() {
    let clock = Arc::new(FakeClock::new(1000));
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Arc::new(EncryptedSkillRuntime::new_with_clock(
        Arc::new(PeriodicSweepSdk),
        TtlConfig {
            skill_idle: Duration::from_secs(5),
        },
        tmp.path().join("mem-root"),
        clock.clone(),
    ));
    let token = runtime
        .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
        .unwrap();
    assert!(!runtime.decrypted_dirs("t1").is_empty());

    let _task = spawn_periodic_sweep(
        tokio::runtime::Handle::current(),
        Arc::downgrade(&runtime),
        Duration::from_millis(10),
    );

    // 空闲 6s（无任何请求/turn 活动），仅由后台周期任务驱动 sweep。
    clock.advance(6_000);
    tokio::time::advance(Duration::from_secs(6)).await;
    tokio::task::yield_now().await;

    assert!(
        runtime.decrypted_dirs("t1").is_empty(),
        "periodic sweep should unload the idle skill"
    );
    assert_eq!(
        runtime.rehydrate_framed(Some("t1"), &token),
        token,
        "evicted token must stay stale"
    );
}

#[tokio::test(start_paused = true)]
async fn periodic_sweep_keeps_fresh_skills_loaded() {
    let clock = Arc::new(FakeClock::new(1000));
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Arc::new(EncryptedSkillRuntime::new_with_clock(
        Arc::new(PeriodicSweepSdk),
        TtlConfig {
            skill_idle: Duration::from_secs(60),
        },
        tmp.path().join("mem-root"),
        clock.clone(),
    ));
    runtime
        .load_or_register("t1", "secret", Path::new("/skills/secret.zip.enc"))
        .unwrap();

    let _task = spawn_periodic_sweep(
        tokio::runtime::Handle::current(),
        Arc::downgrade(&runtime),
        Duration::from_millis(10),
    );

    clock.advance(30_000);
    tokio::time::advance(Duration::from_secs(30)).await;
    tokio::task::yield_now().await;

    assert_eq!(
        runtime.decrypted_dirs("t1").len(),
        1,
        "fresh skill must survive periodic sweeps"
    );
}

#[tokio::test(start_paused = true)]
async fn periodic_sweep_task_exits_when_runtime_is_dropped() {
    let clock = Arc::new(FakeClock::new(1000));
    let tmp = tempfile::tempdir().unwrap();
    let runtime = Arc::new(EncryptedSkillRuntime::new_with_clock(
        Arc::new(PeriodicSweepSdk),
        TtlConfig {
            skill_idle: Duration::from_secs(60),
        },
        tmp.path().join("mem-root"),
        clock.clone(),
    ));
    let task = spawn_periodic_sweep(
        tokio::runtime::Handle::current(),
        Arc::downgrade(&runtime),
        Duration::from_millis(10),
    );

    // The runtime must not be kept alive by the background task: once the
    // session's strong reference is gone, the next tick exits the task.
    drop(runtime);
    tokio::time::advance(Duration::from_millis(100)).await;
    tokio::task::yield_now().await;

    assert!(
        task.is_finished(),
        "periodic sweep task must exit when the runtime is dropped"
    );
}
