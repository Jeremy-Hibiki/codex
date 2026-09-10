use std::path::Path;
use std::path::PathBuf;

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
    // UKey SDK 0.3.2 no longer propagates Linux link flags through the
    // wrapper. The CLI links the SDK itself using metadata from its direct
    // wrapper dependency and selects the mode with FMSH_UKEY_SDK_LINK:
    //
    // - shared (default): the SDK `.so` NEEDs the host's libcrypto.so.3
    //   (SDK 0.3.1), which DT_RUNPATH cannot resolve transitively — mirror
    //   the upstream fmsh-ukey-cli approach and force a direct NEEDED for
    //   libcrypto, pointing the loader at $ORIGIN/lib where the bundled
    //   SDK `.so` files live.
    // - static (release): the SDK archives + vendored libcrypto are
    //   embedded; no libcrypto/SDK `.so` NEEDED at all, so the hack is
    //   skipped. $ORIGIN/lib still resolves the dlopened GM3000 provider
    //   when it is bundled next to the binary.
    //
    // rustc-link-arg-bins from dependency crates never reaches this final
    // link, so the rpath must be re-emitted here.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux")
        && std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("x86_64")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu")
    {
        println!("cargo:rerun-if-env-changed=FMSH_UKEY_SDK_LINK");
        let sdk_static = std::env::var("FMSH_UKEY_SDK_LINK").as_deref() == Ok("static");
        let lib_dir = required_env("DEP_FMSH_UKEY_SDK_LIB_DIR");
        if sdk_static {
            emit_static_ukey_sdk(&lib_dir);
        } else {
            emit_shared_ukey_sdk(&lib_dir);
        }
        if !sdk_static {
            println!("cargo:rustc-link-arg-bins=-Wl,--no-as-needed");
            println!("cargo:rustc-link-arg-bins=-l:libcrypto.so.3");
            println!("cargo:rustc-link-arg-bins=-Wl,--as-needed");
        }
        println!("cargo:rustc-link-arg-bins=-Wl,-rpath,$ORIGIN/lib");
    }
}

fn emit_shared_ukey_sdk(lib_dir: &str) {
    println!("cargo:rustc-link-search=native={lib_dir}");
    println!("cargo:rustc-link-lib=dylib=fmsh_ukey_sdk");
}

fn emit_static_ukey_sdk(lib_dir: &str) {
    let archives = [
        prepare_static_archive(lib_dir, "libfmsh_ukey_sdk.a"),
        prepare_static_archive(lib_dir, "liblmUtils.a"),
        prepare_static_archive(lib_dir, "libPLOG.a"),
    ];
    for archive in archives {
        println!("cargo:rustc-link-arg-bins={}", archive.display());
    }

    println!("cargo:rerun-if-env-changed=FMSH_UKEY_LIBSTDCPP");
    match std::env::var("FMSH_UKEY_LIBSTDCPP").as_deref() {
        Ok("static") | Err(_) => {
            if let Some(dir) = gcc_static_libstdcpp_dir() {
                println!("cargo:rustc-link-search=native={}", dir.display());
            }
            println!("cargo:rustc-link-lib=static=stdc++");
        }
        Ok("shared") => println!("cargo:rustc-link-lib=dylib=stdc++"),
        Ok(other) => {
            panic!("invalid FMSH_UKEY_LIBSTDCPP={other:?}; expected \"static\" or \"shared\"")
        }
    }
}

fn prepare_static_archive(lib_dir: &str, name: &str) -> PathBuf {
    let source = Path::new(lib_dir).join(name);
    assert!(
        source.is_file(),
        "{} not found in {lib_dir} (FMSH_UKEY_SDK_LINK=static)",
        source.display()
    );
    let out_dir = PathBuf::from(required_env("OUT_DIR"));
    let copy = out_dir.join(name);
    std::fs::copy(&source, &copy).unwrap_or_else(|error| {
        panic!(
            "failed to copy {} to {}: {error}",
            source.display(),
            copy.display()
        )
    });
    println!("cargo:rerun-if-changed={}", source.display());
    copy
}

fn gcc_static_libstdcpp_dir() -> Option<PathBuf> {
    let output = std::process::Command::new("cc")
        .arg("-print-file-name=libstdc++.a")
        .output()
        .ok()?;
    let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
    path.is_file()
        .then(|| path.parent().map(Path::to_path_buf))
        .flatten()
}

fn required_env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|error| panic!("failed to read {name}: {error}"))
}
