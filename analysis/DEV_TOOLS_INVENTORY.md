# Codex Tools Inventory

Comprehensive inventory of tools, build systems, dependencies, CLIs, SDKs, and developer tooling used in the Codex project.

## 1. Build System

| Tool | Version / Source | Usage |
|------|-----------------|-------|
| **Bazel** | `MODULE.bazel` (Bzlmod) | Primary hermetic build system for Rust workspace (experimental, since ~6/2026). Provides cross-platform artifact generation, remote execution via BuildBuddy, and Bazel-native test sharding. |
| **Cargo** | Rust 1.95.0, workspace resolver = "2" | Source of truth for Rust crates and features. Bazel imports from `Cargo.toml`/`Cargo.lock` via `rules_rust` `crate.from_cargo()`. |
| **Just** | `justfile` at repo root | Task runner for dev scripts. Wraps cargo, bazel, python, rustfmt, clippy, etc. |
| **Nix** | `default.nix` | Nixpkgs-based build for codex-rs (used in NixOS / development containers). |
| **Bazel + rules_rust** | `rules_rs@0.0.96`, `rules_cc@0.2.18`, `bazel_skylib@1.9.0` | Rust toolchains and crate resolution in Bazel. |
| **Bazel + llvm** | `llvm@0.8.11` (patched) | Hermetic LLVM toolchain for cross-compilation (MSVC, gnullvm). |
| **Bazel + apple_support** | `apple_support@2.1.0` | macOS SDK packaging for Bazel. |
| **Bazel + aws-lc** | `aws-lc@5.1.0.bcr.1` | AWS crypto library for Bazel builds. |
| **Bazel + zstd** | `zstd@1.5.7` | zstd compression library for Bazel. |
| **Bazel + bzip2** | `bzip2@1.0.8.bcr.3` | bzip2 compression for Bazel. |
| **Rust Toolchain** | `rust-toolchain.toml` → 1.95.0, components: `clippy`, `rustfmt`, `rust-src` | Pin: `channel = "1.95.0"`. Nightly `2025-09-18` used for argument-comment-lint via rustc_private. |

## 2. Code Quality & Formatting

| Tool | Usage |
|------|-------|
| **rustfmt** | Configured via `rustfmt.toml` (`imports_granularity = "Item"`) |
| **clippy** | Configured via `clippy.toml`; allows expect/unwrap in tests, disallows `ratatui::Color::Rgb/Index/white/black/yellow`, disallows direct `sqlx::Pool` connections |
| **cargo-deny** | `deny.toml` — advisory checks, license checks, dependency bans (async-trait, reqwest wrapper pattern), multiple-versions warning |
| **cargo-shear** | `.github/workflows/rust-ci.yml` — unused dependency detection (`cargo shear --deny-warnings`) |
| **cargo-audit** | `.cargo/audit.toml` — RustSec advisory ignores (derivative, fxhash, paste, atomic-polyfill, yaml-rust, bincode, hickory-proto, proc-macro-error2, quick-xml) |
| **buildifier** | `tools/buildifier` (DotSlash-wrapped) — Starlark/BUILD file formatting |
| **Ruff** | Python formatting and linting (SDK `pyproject.toml`, `scripts/` directory). Uses Ruff `>=0.15.8` |
| **dotslash** | Windows shebang fallback, used to invoke `tools/buildifier` |
| **Prettier** | TypeScript SDK (`sdk/typescript/package.json`) — `prettier --check .` |
| **ESLint** | TypeScript SDK (`eslint@^9.36.0`, `typescript-eslint@^8.45.0`, `eslint-plugin-jest`, etc.) |
| **Dylint (argument-comment-lint)** | `tools/argument-comment-lint/` — Dylint-based lint for argument comments, built with nightly toolchain + rustc-dev |
| **codespell** | `.github/workflows/codespell.yml` — spelling check in PRs |

## 3. Testing Infrastructure

| Tool | Version / Source | Usage |
|------|-----------------|-------|
| **cargo-nextest** | `just test *args` → `cargo nextest run --no-fail-fast` | Primary test runner. Config: `.config/nextest.toml` with JUnit output, retry (1), slow-timeout (30s), test-grouped concurrency limits |
| **cargo bench** | `just bench` → `cargo bench --workspace --bench '*'` | Workspace benchmark runner |
| **Bazel test** | `bazel test //...` via `.github/workflows/bazel.yml` | Bazel-native test execution with sharding, BuildBuddy cache, remote execution |
| **Bazel e2e-benchmarks** | `//codex-rs:e2e-benchmarks` (manual tag) | Bazel-backed macrobenchmarks |
| **pytest** | `sdk/python/pyproject.toml` (pytest >= 8.0) | Python SDK test suite |
| **Jest** | `sdk/typescript/package.json` (jest ^29.7.0) | TypeScript SDK test suite |
| **assert_cmd** | Rust workspace dep | CLI integration test framework |
| **insta** | Snapshots via `INSTA_WORKSPACE_ROOT` env var | Snapshot testing (Bazel + Cargo parity) |
| **serial_test** | Rust dev-dep | Serialized test execution |
| **test-case** | `test-case = "3.3.1"` | Parameterized tests |
| **predicates** | Rust dev-dep | Text matching in integration tests |
| **wiremock** | Rust dev-dep | HTTP mock server for tests |

## 4. Language & Runtime

| Language / Runtime | Version / Source | Details |
|-------------------|-----------------|---------|
| **Rust** | Edition 2024, toolchain 1.95.0 | Primary language. ~80+ crates in workspace |
| **Python** | >=3.10 | SDK (`sdk/python/pyproject.toml`), CI scripts, formatting via Ruff, test via pytest |
| **TypeScript** | >=5.9.2, Node >=18 | SDK (`sdk/typescript/package.json`) |
| **Node.js** | >=18 | Runtime for TypeScript SDK build |
| **Bun** | — | Not used in codex-rs (different repo) |

## 5. Package Managers

| Tool | Source | Usage |
|------|--------|-------|
| **cargo** | Rust workspace | Rust dependency management |
| **pnpm** | `sdk/typescript/package.json` (`pnpm@10.33.0`) | TypeScript SDK |
| **npm** | Implicit (pnpm invokes) | TypeScript SDK build |
| **uv** | `scripts/uv.lock`, `sdk/python/pyproject.toml` | Python SDK + scripts tooling (Ruff, format, test) |

## 6. External CLIs Invoked at Runtime

These are the external command-line tools that Codex spawns via `std::process::Command::new()` or the `which` crate:

| CLI Tool | Source | Used By |
|----------|--------|---------|
| **bubblewrap / bwrap** | Vendored source at `codex-rs/vendor/bubblewrap/`; also compiled in-tree via `codex-rs/bwrap/` (CC build, libcap) | `codex-linux-sandbox/` — Linux sandbox filesystem isolation. Key: `--ro-bind`, `--bind`, `--unshare-user`, `--unshare-pid`, `--unshare-net`, `--proc`. Also `codex-cli/src/desktop_app/mac.rs` for macOS installer download. |
| **curl** | `codex-cli/src/desktop_app/mac.rs` | macOS installer download |
| **git** | System PATH, via `which` crate in `codex-git-utils/` | Git operations (clone, init, config, add, commit, remote, push, rev-parse, checkout) in `core/`, `cloud-tasks/`, `chatgpt/`, `core-plugins/`, `app-server/` |
| **python / python3** | `justfile` (`python := python3 | python`) | CI scripts, formatting (`scripts/format.py`), Ruff, test discovery |
| **bash / sh** | System PATH | Shell command execution via `Codex` shell escalation |
| **ps** | System PATH | Process inspection (`app-server-daemon/pid.rs`, `app-server/tests/suite/v2/command_exec.rs`) |
| **kill** | System PATH | Process termination (`app-server/tests/suite/v2/connection_handling_websocket_unix.rs`, `code-mode-host/tests/stdio.rs`) |
| **mkfifo** | System PATH | Named pipe creation in tests (`app-server/tests/suite/external_agent_config.rs`, `app-server/tests/suite/fs.rs`) |
| **lsof** | System PATH (macOS/Linux) | Port binding detection in `app-server-test-client/` |
| **sleep** | System PATH | Test fixture (`app-server/tests/suite/v2/pid_tests.rs`) |

## 7. External CLIs Bundled / Ship-With

| Tool | Form | Details |
|------|------|---------|
| **bwrap (bubblewrap)** | Pre-compiled binary at `codex-resources/bwrap` | Bundled alongside the `codex` binary for sandboxing. Built from vendored bubblewrap C source via `cc` crate build script. SHA256 verified via `CODEX_BWRAP_SHA256`. |
| **codex** | Self | The main CLI binary (`codex-cli`) |

## 8. Runtime Dependencies (C Rust Crates)

Key Rust crates that wrap or interact with external system tools:

| Rust Crate | External Dependency |
|-----------|-------------------|
| `codex_utils_cargo_bin` | Binary path resolution for Cargo binaries (`cargo_bin`, `find_resource`) — **preferred over escargot** per AGENTS.md |
| `portable-pty` | PTY management (Unix terminals) |
| `crossterm` | Terminal control / ANSI |
| `arboard` | Clipboard (wayland-data-control feature) |
| `syntect` | Syntax highlighting (syntactically via `sublime-text-syntaxes`) |
| `syntect` → `yaml-rust`, `bincode` | Text file parsing |
| `landlock` | Linux Landlock sandbox API |
| `seccompiler` | Linux seccomp-bpf compilation |
| `tree-sitter`, `tree-sitter-bash` | Bash AST parsing (shell-command, apply-patch) |
| `starlark` | Starlark VM (execpolicy) |
| `v8` | V8 JavaScript engine (`codex-rs/v8-poc/`) |
| `rmcp` | Model Context Protocol client/server |
| `reqwest` | HTTP client (migrated toward `codex-http-client`) |
| `tokio-tungstenite` | WebSocket client/server |
| `sqlx` | SQLite (via `codex-state` shim) |
| `notify` | File system watching |
| `ignore` | Glob matching (ripgrep-like) |
| `clap` / `clap_complete` | CLI argument parsing + shell completions |

## 9. Third-Party / External C Libraries

| Library | Used By | Build System |
|---------|---------|-------------|
| **libcap** | `bwrap/` build (vendored bubblewrap) | `pkg-config` (Cargo build), direct C compilation |
| **openssl** | `default.nix` (Nix build), many Rust crates via `openssl-sys` | Nix, Cargo |
| **zstd** | `codex-http-client/`, `cli` test deps | Bazel (`zstd@1.5.7`), Cargo |
| **bzip2** | Bazel dependency | Bazel (`bzip2@1.0.8.bcr.3`) |
| **v8** | `codex-rs/v8-poc/` | Bazel (`third_party/v8/`), hermetic LLVM |
| **AWS-LC** | `aws-lc-sys` / `aws-lc-rs` | Bazel + Cargo patches |

## 10. Developer SDKs & Services

### CLI / Application Binaries

| Binary | Crate | Purpose |
|--------|-------|---------|
| **codex** | `codex-cli` | Main CLI binary |
| **codex-tui** | `codex-tui` | Terminal UI (Ratatui-based) |
| **codex-app-server** | `codex-app-server` | WebSocket-based app server |
| **exec-server** | `codex-app-server/src/bin/exec_server.rs` | Exec server (subprocess management) |
| **codex-exec** | `codex-exec` | Standalone exec CLI |
| **codex-apply-patch** | `codex-apply-patch` | Patch application CLI |
| **codex-code-mode-host** | `codex-code-mode-host` | Code-mode WebSocket host |
| **codex-mcp-server** | `codex-mcp-server` | MCP (Model Context Protocol) server |
| **codex-response-proxy** | `responses-api-proxy/` | Responses API proxy |
| **logs_client** | `codex-cli` | SQLite log reading CLI |
| **codex-execpolicy** | `codex-execpolicy` | Exec policy CLI (Starlark rules) |
| **codex-linux-sandbox** | `codex-linux-sandbox` | Linux sandbox helper binary |

### SDK Packages

| SDK | Language | Location |
|-----|----------|----------|
| **openai-codex** | Python | `sdk/python/` (pydantic, uv, pytest) |
| **@openai/codex-sdk** | TypeScript | `sdk/typescript/` (tsup, jest, pnpm, zod) |
| **openai-codex-cli-bin** | Python pip package | Published binary dependency for Python SDK |

### Protocol / API Specifications

| Spec | Source | Purpose |
|------|--------|---------|
| **app-server-protocol** | `app-server-protocol/` | JSON + TypeScript schema generation (`codex-app-server-protocol`) |
| **exec-server-protocol** | `exec-server-protocol/` | gRPC/protobuf protocol |
| **code-mode-protocol** | `code-mode-protocol/` | Code-mode WebSocket protocol |
| **codex-backend-openapi-models** | `codex-backend-openapi-models/` | OpenAPI model generation |

### Extension Crates

| Extension | Crate | Purpose |
|-----------|-------|---------|
| agent | `ext/agent/` | Agent extension API |
| connectors | `ext/connectors/` | Connector extension API |
| extension-api | `ext/extension-api/` | Extension API surface |
| goal | `ext/goal/` | Goal extension |
| git-attribution | `ext/git-attribution/` | Git attribution extension |
| guardian | `ext/guardian/` | Guardian extension |
| image-generation | `ext/image-generation/` | Image generation extension |
| items | `ext/items/` | Items extension |
| memories | `ext/memories/` | Memories extension |
| mcp | `ext/mcp/` | MCP extension |
| skills | `ext/skills/` | Skills extension |
| web-search | `ext/web-search/` | Web search extension |

## 11. CI / GitHub Actions

| Workflow | Purpose |
|----------|---------|
| `rust-ci.yml` | Fast Cargo-native PR checks (fmt, bench-smoke, cargo-shear, argument-comment-lint) |
| `rust-ci-full.yml` | Full CI (all platforms, all crates) |
| `rust-ci-full-nextest-platform.yml` | nextest platform-specific tests |
| `bazel.yml` | Bazel build/test on macOS, Linux (x86_64, musl), Windows |
| `rust-release.yml` | Rust release pipeline |
| `rust-release-zsh.yml` | Zsh completion release |
| `rusty-v8-release.yml` | V8 release management |
| `v8-canary.yml` | V8 canary builds |
| `python-runtime-build.yml` | Python SDK binary build |
| `python-runtime-release.yml` | Python SDK release |
| `python-sdk-release.yml` | Python SDK release pipeline |
| `sdk.yml` | SDK release workflow |
| `cargo-deny.yml` | cargo-deny security/license checks |
| `blob-size-policy.yml` | Blob size enforcement |
| `codespell.yml` | Spelling check |
| `r2-release.yml` | R2 (object storage) release |

## 12. Remote / Cloud Infrastructure

| Service | Source | Purpose |
|---------|--------|---------|
| **BuildBuddy** | `BUILDBUDDY_API_KEY` env var | Remote Bazel cache, remote execution, build event streaming |
| **AWS** | `aws-config`, `aws-credential-types`, `aws-sigv4`, `aws-auth/` | Cloud tasks, authentication |
| **Cloud Tasks** | `cloud-tasks/`, `cloud-tasks-client/`, `cloud-tasks-mock-client/` | Async task queue |
| **R2 / Object Storage** | `r2-release.yml` | Artifact storage |

## 13. CI Runner Environments

| Platform | Runner | Notes |
|----------|--------|-------|
| Linux x86_64 | `ubuntu-24.04` | Default Bazel + Cargo CI |
| Linux arm64 | `ubuntu-24.04-arm` | Disabled for Bazel (flaky) |
| macOS | `macos-15-xlarge` | Apple Silicon + x86_64 Bazel targets |
| Windows | `group: codex-runners / labels: codex-windows-x64` | Custom runners, sharded Bazel tests |
| Windows x64 (native) | Same group | Post-merge main branch tests |

## 14. Shell / Script Tooling

| Script | Purpose |
|--------|---------|
| `scripts/format.py` | Unified formatter: just, rustfmt, buildifier, Ruff (SDK + scripts) |
| `scripts/just-shell.py` | Custom shell for `just` (cross-platform) |
| `scripts/run_tui_with_exec_server.sh` | Launch exec-server + TUI workflow |
| `scripts/start-codex-exec.sh` | Start codex exec |
| `scripts/debug-codex.sh` | Debug helper |
| `scripts/test-remote-env.sh` | Remote environment testing |
| `scripts/check-module-bazel-lock.sh` | Bazel lockfile validation |
| `scripts/mock_responses_websocket_server.py` | Test mock server |
| `scripts/asciicheck.py` | ASCII validation |
| `scripts/readme_toc.py` | README table of contents generation |
| `scripts/build_codex_package.py` | Package building |
| `scripts/check_blob_size.py` | Blob size checking |
| `scripts/stage_npm_packages.py` | NPM package staging |
| `tools/argument-comment-lint/run.py` | Dylint runner for argument-comment-lint |
| `tools/argument-comment-lint/run-prebuilt-linter.py` | Prebuilt linter wrapper |
| `.github/scripts/run_bazel_with_buildbuddy.py` | Bazel wrapper for BuildBuddy |
| `.github/scripts/run-bazel-ci.sh` | CI Bazel launcher |
| `.github/scripts/rusty_v8_bazel.py` | V8 MODULE.bazel checksum validation |
| `scripts/just-shell.py` | Justfile cross-platform shell |

## 15. Container / OS-Level

| Tool | Usage |
|------|-------|
| **bubblewrap** | Primary Linux sandbox (user namespaces, mount isolation, seccomp). Bundled + system-path fallback. |
| **Landlock** | Linux Landlock LSM for legacy sandbox fallback (`features.use_legacy_landlock`) |
| **seccomp** | `seccompiler` crate for seccomp-bpf network filtering in sandbox |
| **PR_SET_NO_NEW_PRIVS** | Applied in-process during bubblewrap sandboxing |
| **Windows Sandboxing** | `codex-windows-sandbox` crate — Seatbelt, restricted tokens, private desktops |

## 16. Source Versioning

| Tool | Source | Usage |
|------|--------|-------|
| **git** | `codex-git-utils/` | Git operations (clone, status, diff, log, config, remote, push) |
| **git-cliff** (implied) | Release tag management | Version bumping and changelog generation |
| **GitHub MCP** | GitHub actions (`actions/checkout@v6`, `actions/cache@v5`, `actions/upload-artifact@v7`) | CI workflow integration |
| **dtolnay/rust-toolchain** | `actions/dtolnay/rust-toolchain@1.95.0` | Rust toolchain pinning in CI |