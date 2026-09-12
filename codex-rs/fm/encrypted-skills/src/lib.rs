//! Encrypted skill security core for Codex.
//!
//! Implements the non-wiring logic of the `encrypt-agent-security-skills`
//! OpenSpec change (see `openspec/changes/encrypt-agent-security-skills`):
//! token sentinels, the digital-envelope SDK boundary, decrypted directory
//! layout under the memory root, the per-session content cache, the
//! skill-level TTL registry with turn/thread-end cleanup, request-time
//! rehydration with trust-tier framing, path rewriting/redaction, outbound
//! plaintext detection, and audit events.
//!
//! The crate deliberately keeps every pure function free of IO so the host
//! wiring points stay thin:
//!
//! 1. `codex_core_skills::injection::build_skill_injections` — decrypt on
//!    mention, register the session mapping, and emit token placeholders via
//!    `SkillInstructions::body()`.
//! 2. `codex_core::client_common::get_formatted_input_for_request` — rehydrate
//!    sentinel tokens with framed `SKILL.md` content before the request is
//!    transmitted.
//! 3. PreToolUse hook registration — block reads of the memory root, rewrite
//!    original skill paths to decrypted paths, and detect outbound plaintext.

pub mod audit;
pub mod cache;
pub mod export_guard;
pub mod guard;
pub mod guardrail;
pub mod mem_root;
pub mod memfd;
pub mod paths;
pub mod registry;
pub mod rehydrate;
pub mod rpc;
pub mod runtime;
pub mod sandbox_policy;
pub mod sdk;
pub mod session_guard;
pub mod token;

/// Whether the real envelope backends (`fmsh-ukey-core` / the FMSH UKey SDK)
/// are compiled in.
///
/// Compile-time constant: false on non-glibc platforms and whenever the
/// `ukey` feature is off. [`sdk_for`] then returns the fail-closed
/// [`sdk::UnavailableSdk`] for every real backend kind, so encrypted skills
/// fail to load and the runtime never engages. Tests that need real
/// decryption should skip themselves when this returns false.
pub fn ukey_available() -> bool {
    cfg!(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_env = "gnu",
        feature = "ukey"
    ))
}
