fn git(args: &[&str]) -> Option<String> {
    let output = std::process::Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Build suffix `(revision, hash)` derived from `git describe`.
///
/// `git describe --tags --match 'rust-v[0-9]*'` picks the nearest reachable
/// upstream tag, so the revision count follows the currently tracked upstream
/// baseline automatically (for example `rust-v0.147.0-12-ga2b43d0099`). When
/// HEAD is exactly on the tag there is no `-N-g...` segment: revision is `0`
/// and the hash falls back to HEAD.
fn describe_suffix() -> Option<(String, String)> {
    let describe = git(&["describe", "--tags", "--match", "rust-v[0-9]*"])?;
    let describe = describe.strip_prefix("rust-v")?;
    if let Some((_, tail)) = describe.rsplit_once('-')
        && let Some(hash) = tail.strip_prefix('g')
    {
        let (head, _) = describe.rsplit_once('-')?;
        let (_, count) = head.rsplit_once('-')?;
        let hash: String = hash.chars().take(8).collect();
        Some((count.to_string(), hash))
    } else {
        // Exactly on the tag: no commit count segment, hash is HEAD itself.
        Some((
            "0".to_string(),
            git(&["rev-parse", "--short=8", "HEAD"]).unwrap_or_else(|| "unknown".to_string()),
        ))
    }
}

fn main() {
    // Bazel/Docker builds run outside a git checkout (or without the git
    // metadata in the action sandbox), so callers can inject the exact suffix
    // through the FM_BUILD_SUFFIX environment variable (for example
    // `fm.r37-456e4457`). Cargo builds keep deriving it from `git describe`.
    let suffix = std::env::var("FM_BUILD_SUFFIX")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| {
            // Build-version suffix `fm.rNNN-HHHHHHHH`: NNN is the commit count
            // since the nearest reachable `rust-v<version>` tag (git describe
            // semantics), HHHHHHHH is the short commit hash. Falls back to the
            // full history count (or `r0-unknown`) outside a git worktree /
            // without a matching tag.
            let (revision, hash) = describe_suffix().unwrap_or_else(|| {
                (
                    git(&["rev-list", "--count", "HEAD"]).unwrap_or_else(|| "0".to_string()),
                    git(&["rev-parse", "--short=8", "HEAD"])
                        .unwrap_or_else(|| "unknown".to_string()),
                )
            });
            format!("fm.r{revision}-{hash}")
        });
    // Always set cargo:rustc-env so env!("FM_BUILD_SUFFIX") in main.rs can
    // expand at compile time.
    println!("cargo:rustc-env=FM_BUILD_SUFFIX={suffix}");
    // FM_BUILD_SUFFIX is an env-var input to this script. Without this,
    // cargo skips build.rs on incremental builds (no source files changed),
    // baking in the old suffix.
    println!("cargo:rerun-if-env-changed=FM_BUILD_SUFFIX");
    println!("cargo:rerun-if-changed=build.rs");
    if let Some(head_path) = git(&["rev-parse", "--git-path", "HEAD"]) {
        println!("cargo:rerun-if-changed={head_path}");
        if let Some(tags_path) = git(&["rev-parse", "--git-path", "refs/tags"]) {
            println!("cargo:rerun-if-changed={tags_path}");
        }
    }
    // Changing the Cargo.toml version (for example when bumping the upstream
    // baseline) must also rerun this build script.
    println!("cargo:rerun-if-changed=Cargo.toml");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-link-arg=-ObjC");
    }
    // The FMSH UKey SDK (linked transitively via fm-encrypted-skills) is a
    // dynamic library whose NEEDED libcrypto.so.1.1 is not resolvable through
    // DT_RUNPATH (runpath is not transitive). Mirror the upstream
    // fmsh-ukey-enc approach: force a direct NEEDED for libcrypto and point
    // the loader at $ORIGIN/lib, where the bundled SDK .so files live.
    //
    // rustc-link-arg-bins from dependency crates never reaches this final
    // link, so the rpath must be re-emitted here. The .so files themselves
    // are copied next to the binary by the bundle step (see
    // scripts/bundle_sdk.sh in fmsh-ukey-lib).
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux")
        && std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("x86_64")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu")
    {
        println!("cargo:rustc-link-arg-bins=-Wl,--no-as-needed");
        println!("cargo:rustc-link-arg-bins=-l:libcrypto.so.1.1");
        println!("cargo:rustc-link-arg-bins=-Wl,--as-needed");
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN/lib");
    }
}
