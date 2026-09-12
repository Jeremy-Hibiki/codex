//! Placeholder for the internal `fmsh-ukey-sdk-wrapper` crate of
//! `fmsh-ukey-lib` (internal GitLab 192.168.131.126:8089).
//!
//! The real crate vendored the FMSH UKey SDK binaries and published the SDK
//! directory via `links = "fmsh_ukey_sdk"` metadata. This placeholder keeps
//! the same contract (see `build.rs`): it compiles an empty stub shared
//! library so the link directives emitted by downstream build scripts
//! (`fm-encrypted-skills`, `codex-cli`) resolve offline. There is no Rust
//! API — no downstream Rust code uses the wrapper directly.
