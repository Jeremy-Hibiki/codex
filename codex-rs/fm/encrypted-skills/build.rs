//! Links the FMSH UKey SDK for this crate's own test targets — and, in shared
//! mode, for every downstream link target that pulls this rlib in.
//!
//! `fmsh-ukey-sdk-wrapper` deliberately emits only package-scoped link args
//! (`cargo:rustc-link-arg`, which does not propagate downstream), publishing the
//! resolved SDK directory through its `links` key instead. Consumers that link
//! the wrapper's FFI symbols must therefore emit their own directives: without
//! them the link fails with `undefined symbol: FM_Initialize`.
//!
//! Mode selection mirrors `codex-cli`'s build script (the owner of the release
//! binary's final link):
//!
//! - **shared** (dev/test default): propagating `-l fmsh_ukey_sdk` + `-L`. The
//!   rlib metadata carries them into the final link of every dependent — which
//!   is exactly what non-CLI targets need: `grevo-tui`, `grevo-app-server`,
//!   `codex-core` test binaries pull the wrapper's objects and would otherwise
//!   fail to resolve `FM_*`. The `.so` lives in the wrapper's vendor directory,
//!   so the extra NEEDED is harmless outside release distribution.
//! - **static** (release, `FMSH_UKEY_SDK_LINK=static`): NOTHING propagating.
//!   Propagating `-l dylib` here would pin `libfmsh_ukey_sdk.so.0` into
//!   `grevo`'s NEEDED while the static image only bundles `libgm3000.1.0.so`
//!   (runtime "cannot open shared object file"). Only this crate's own test
//!   targets get the archives via package-scoped `rustc-link-arg`; the release
//!   CLI links itself via `codex-cli`'s build script.
//!
//! Static-mode caveat: `cargo test -p <downstream>` with
//! `FMSH_UKEY_SDK_LINK=static` will not link — static is a release-distribution
//! mode; run tests in the default shared mode.

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux")
        || std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("x86_64")
        || std::env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("gnu")
    {
        return;
    }

    // Published by `fmsh-ukey-sdk-wrapper` (it declares `links = "fmsh_ukey_sdk"`).
    let Ok(lib_dir) = std::env::var("DEP_FMSH_UKEY_SDK_LIB_DIR") else {
        panic!(
            "DEP_FMSH_UKEY_SDK_LIB_DIR is unset: `fmsh-ukey-sdk-wrapper` must be a dependency \
             of this crate so its build script publishes the SDK directory"
        );
    };

    println!("cargo:rerun-if-env-changed=FMSH_UKEY_SDK_LINK");
    if std::env::var("FMSH_UKEY_SDK_LINK").as_deref() == Ok("static") {
        emit_static(&lib_dir);
    } else {
        emit_shared(&lib_dir);
    }
}

/// Package-scoped: covers this crate's own test targets only. The archives are
/// compiled without `-fPIC` (executables-only), and `liblmUtils.a`'s unversioned
/// `stat`/`lstat` raise the runtime floor to glibc 2.33.
fn emit_static(lib_dir: &str) {
    // Keeps test binaries runnable without LD_LIBRARY_PATH: the dlopened
    // GM3000 provider sits next to the archives.
    println!("cargo:rustc-link-arg=-Wl,-rpath,{lib_dir}");
    println!("cargo:rustc-link-arg=-L{lib_dir}");
    for archive in ["libfmsh_ukey_sdk.a", "liblmUtils.a", "libPLOG.a"] {
        println!("cargo:rustc-link-arg={lib_dir}/{archive}");
    }
    // C++ runtime for the archives. Release builds keep libstdc++ dynamic
    // (`FMSH_UKEY_LIBSTDCPP=shared`): statically embedding it clashes with
    // the libc++abi V8 embeds on the __cxa_* ABI symbols.
    println!("cargo:rustc-link-arg=-lstdc++");
}

/// Propagating: baked into this crate's rlib metadata and replayed at every
/// dependent's final link. A package-scoped `-l` here would leave
/// `grevo-tui`/`grevo-app-server`/`codex-core` test binaries with undefined
/// `FM_*` symbols (only `codex-cli` links the SDK itself).
fn emit_shared(lib_dir: &str) {
    println!("cargo:rustc-link-search=native={lib_dir}");
    println!("cargo:rustc-link-lib=dylib=fmsh_ukey_sdk");
    // Package-scoped rpath so this crate's own test binaries resolve the SDK's
    // SONAME (`libfmsh_ukey_sdk.so.0`) without LD_LIBRARY_PATH.
    println!("cargo:rustc-link-arg=-Wl,-rpath,{lib_dir}");
}
