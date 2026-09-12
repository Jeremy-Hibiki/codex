//! Links the FMSH UKey SDK for this crate's own test targets.
//!
//! `fmsh-ukey-sdk-wrapper` deliberately emits only package-scoped link args
//! (`cargo:rustc-link-arg`, which does not propagate downstream) so each final
//! artifact picks its own link mode. It does, however, publish the resolved SDK
//! directory through its `links` key. Consumers that link the wrapper's FFI
//! symbols must therefore emit their own directives: without them the test
//! binary fails to link with `undefined symbol: FM_Initialize`.
//!
//! Only test targets need this: production binaries get their link directives
//! from `codex-cli`'s build script, which owns the final link.
//!
//! Release builds set `FMSH_UKEY_SDK_LINK=static` (see `release/`), which is
//! out of scope here — the SDK archives are compiled without `-fPIC`, so static
//! linking is executables-only.

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

    println!("cargo:rustc-link-search=native={lib_dir}");
    println!("cargo:rustc-link-lib=dylib=fmsh_ukey_sdk");
    // The SDK's SONAME is `libfmsh_ukey_sdk.so.0`, which is absent from the
    // default loader path; bake the rpath so tests run without LD_LIBRARY_PATH.
    println!("cargo:rustc-link-arg=-Wl,-rpath,{lib_dir}");
    // The shared SDK needs the host's libcrypto.so.3, dropped by --as-needed
    // because nothing in the Rust code references it directly.
    println!("cargo:rustc-link-arg=-Wl,--no-as-needed");
    println!("cargo:rustc-link-arg=-l:libcrypto.so.3");
    println!("cargo:rustc-link-arg=-Wl,--as-needed");
}
