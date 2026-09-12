//! Builds the stub `libfmsh_ukey_sdk` shared library and publishes its
//! directory through the `links = "fmsh_ukey_sdk"` metadata key, exactly like
//! the real wrapper does (`DEP_FMSH_UKEY_SDK_LIB_DIR`).
//!
//! The real wrapper ships the vendored FMSH UKey SDK binaries; the offline
//! placeholder ships an empty translation unit — nothing references the SDK
//! FFI symbols because `fmsh-ukey-core` is also a placeholder. Downstream
//! build scripts (`fm-encrypted-skills`, `codex-cli`) still emit their own
//! `-l fmsh_ukey_sdk` directives, which resolve against this stub archive.

fn main() {
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo");
    cc::Build::new()
        .file("stub/fmsh_ukey_sdk_stub.c")
        .compile("fmsh_ukey_sdk");
    println!("cargo:lib_dir={out_dir}");
    // Package-scoped link args (matching the real wrapper): they apply to
    // this crate's own compilation and deliberately do not propagate.
    println!("cargo:rustc-link-search=native={out_dir}");
    println!("cargo:rustc-link-lib=dylib=fmsh_ukey_sdk");
}
