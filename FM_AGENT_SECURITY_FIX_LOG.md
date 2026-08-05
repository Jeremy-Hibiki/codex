# fm/agent-security review fix log

Each entry records one review finding and the commit that fixed it. Bazel/CI
items were excluded per request; the skill-level TTL stays single-tier by
design.

| # | Commit | Scope | Fix |
|---|--------|-------|-----|
| 1 | `43d91df7ae` | `fm/encrypted-skills` token | Require 32-hex-char token keys, reject ambiguous session ids (`:`, `]`); replace per-byte `format!` with `write!` in `hex_encode` |
| 2 | `4abe4cfef4` | `fm/encrypted-skills` SDK | Cap package at 512 entries and 16 MiB decompressed bytes to close the zip-bomb/OOM window |
| 3 | `18f0ea9358` | `fm/encrypted-skills` audit | serde_json JSONL (escaping), `timestamp_ms` field, 0600 file mode, log rotation/write/flush failures |
| 4 | `a50294cb3c` | `fm/encrypted-skills` mem root | Mutex-serialized one-time init; chmod per-process namespace dir 0700 |
| 5 | `a98c559015` | paths + shell guard | Block script-execution segments that read guarded storage via `<`, `$(…)`, backticks, here-strings, process substitution; keep plain resource args allowed |
| 6 | `77394d1143` | cache/registry/runtime | Cache never evicts registry-referenced tokens; `register` returns the replaced record and `load_or_register` wipes replaced/partial dirs; remove dead public API; docs match single-tier TTL |
| 7 | `6594611221` | periodic sweep | Sweep task holds `Weak<EncryptedSkillRuntime>` so it exits when the session runtime drops instead of leaking it |
| 8 | `31cfaea079` | fork isolation | Strip sentinel tokens from all forked text surfaces (assistant/tool/reasoning, inter-agent messages, compacted history, function args) |
| 9 | `9d47154608` | skill injection | Honor `metadata.encryption.package`; fall back to `<name>.zip.enc` |
| 10 | `5864546904` | fm-license | `verify_at_startup` returns `Result<LicenseGuard>` instead of an always-`Some` `Option` |
| 11 | `e28a313c46` | audit sink | Log audit events dropped because the sink mutex was poisoned |
| 12 | `52a9289e22` | session wiring | `tracing::warn!` when the simulated `test_zip` envelope SDK is selected |
| 13 | `6c94e649e7` | rehydrate API | Remove unused `Runtime::rehydrate`; gate unframed helper to `cfg(test)` |
| 14 | `9119204ba7` | OpenSpec | Align access-control spec with shell-only decrypted-storage channel |

## Verification

- `just test -p fm-encrypted-skills`: 124 passed.
- `just test -p codex-core` (`encrypted_skills` + `spawn` filters): 66 + 103 passed.
- `just test -p codex-core-skills` (excluding two pre-existing environment-only
  failures in this workspace): 135 passed.
- `just test -p fm-license`: passed.
- `cargo check -p codex-cli`: passed.

After the I11–I14 round: `fm-encrypted-skills` 124 passed and the
`codex-core` `spawn` filter 102 passed (one approvals test is flaky in this
workspace and passed on the identical code in the previous run).

Two pre-existing loader tests fail identically before and after these changes
when run inside this git worktree (`non_git_repo_skills_search_does_not_walk_parents`,
`skill_roots_include_admin_with_lowest_priority`); they are environment-dependent
and unrelated to this fix set.

## Deliberately not changed

- Bazel/CI items (MODULE.bazel.lock, workspace `zip` default-features): excluded
  per request.
- Thread-level TTL: single skill-level TTL is by design.
- `TestZipSdk` remains selectable as the documented simulated envelope, but
  selecting it now emits a loud startup warning.
- Non-shell tool relative-path rewriting: the access-control spec was updated
  to match the implemented shell-only channel decision.
