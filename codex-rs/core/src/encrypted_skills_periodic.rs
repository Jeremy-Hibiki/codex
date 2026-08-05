//! Periodic encrypted-skill TTL sweep for long-lived processes.

use std::sync::Arc;
use std::time::Duration;

use fm_encrypted_skills::runtime::EncryptedSkillRuntime;
use tokio::runtime::Handle;

/// Default interval for the idle sweep (30s).
pub(crate) const PERIODIC_SWEEP_INTERVAL: Duration = Duration::from_secs(30);

/// Spawns a background task that runs the two-tier TTL sweep every `interval`.
///
/// Covers the "process alive but idle" window that turn-boundary and
/// request-level sweeps cannot reach. A hung process still requires the
/// deployment-side watchdog (healthcheck + restart) to be cleaned up.
pub(crate) fn spawn_periodic_sweep(
    handle: Handle,
    runtime: Arc<EncryptedSkillRuntime>,
    interval: Duration,
) -> tokio::task::JoinHandle<()> {
    handle.spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            ticker.tick().await;
            runtime.sweep();
        }
    })
}

#[cfg(test)]
#[path = "encrypted_skills_periodic_tests.rs"]
mod tests;
