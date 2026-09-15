//! Links the FMSH UKey SDK for this crate's own test targets.
//!
//! `fmsh-ukey-sdk-wrapper` deliberately emits only package-scoped link args
//! (`cargo:rustc-link-arg`, which does not propagate downstream) so each final
//! artifact picks its own link mode. It does, however, publish the resolved SDK
//! directory through its `links` key. Consumers that link the wrapper's FFI
//! symbols must therefore emit their own directives: without them the test
//! binary fails to link with `undefined symbol: FM_Initialize`.
//!
//! Every directive here is package-scoped for the same reason. `-l`/`-L` from a
//! build script propagate into the final link of every dependent, so emitting
//! the shared SDK here would pin `libfmsh_ukey_sdk.so.0` into `grevo`'s NEEDED
//! even when `FMSH_UKEY_SDK_LINK=static` asks for the static release link —
//! `codex-cli`'s build script owns that final link (see `release/Dockerfile`).
//!
//! Static mode links the SDK archives instead of the shared library. The
//! archives are compiled without `-fPIC`, so static linking is
//! executables-only (test binaries and the release CLI), and `liblmUtils.a`'s
//! unversioned `stat`/`lstat` raise the runtime floor to glibc 2.33.

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

    // Keeps test binaries runnable without LD_LIBRARY_PATH: shared mode resolves
    // the SDK's SONAME (`libfmsh_ukey_sdk.so.0`) from here, static mode the
    // dlopened GM3000 provider that sits next to the archives.
    println!("cargo:rustc-link-arg=-Wl,-rpath,{lib_dir}");
    println!("cargo:rustc-link-arg=-L{lib_dir}");

    println!("cargo:rerun-if-env-changed=FMSH_UKEY_SDK_LINK");
    if std::env::var("FMSH_UKEY_SDK_LINK").as_deref() == Ok("static") {
        for archive in ["libfmsh_ukey_sdk.a", "liblmUtils.a", "libPLOG.a"] {
            println!("cargo:rustc-link-arg={lib_dir}/{archive}");
        }
        // C++ runtime for the archives. Release builds keep libstdc++ dynamic
        // (`FMSH_UKEY_LIBSTDCPP=shared`): statically embedding it clashes with
        // the libc++abi V8 embeds on the __cxa_* ABI symbols.
        println!("cargo:rustc-link-arg=-lstdc++");
        return;
    }

    println!("cargo:rustc-link-arg=-lfmsh_ukey_sdk");
    // The shared SDK needs the host's libcrypto.so.3, dropped by --as-needed
    // because nothing in the Rust code references it directly.
    println!("cargo:rustc-link-arg=-Wl,--no-as-needed");
    println!("cargo:rustc-link-arg=-l:libcrypto.so.3");
    println!("cargo:rustc-link-arg=-Wl,--as-needed");
}
