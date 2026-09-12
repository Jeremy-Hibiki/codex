//! Sandbox policy predicates for encrypted-skill sessions.
//!
//! Decides whether an engaged session is running under a sandbox at all.
//! Kept separate from [`crate::guard`] (tool-call interception) so neither
//! module needs to grow with the other.

/// Product policy (I6): engaged sessions must execute under an active
/// sandbox; full-access execution is rejected at runtime unless the
/// debug-only bypass is active.
pub fn ensure_encrypted_skill_sandbox(
    engaged: bool,
    sandbox_requested: bool,
    allow_sandbox_bypass: bool,
) -> Result<(), &'static str> {
    if engaged && !sandbox_requested && !allow_sandbox_bypass {
        Err("encrypted skills require an active sandbox; full-access execution is disabled")
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "sandbox_policy_tests.rs"]
mod tests;
