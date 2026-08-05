use std::path::Path;
use std::path::PathBuf;

/// Links against the FMSH UKey SDK shared library and embeds an RPATH so the
/// SDK and its transitive libcrypto.so.1.1 resolve without LD_LIBRARY_PATH.
///
/// SDK directory resolution:
///   1. FMSH_UKEY_SDK_DIR (absolute or relative to the workspace root)
///
/// No fallback is provided; the variable must be set at build time.
fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace_root = manifest_dir
        .join("..")
        .join("..")
        .canonicalize()
        .expect("workspace root must exist");

    let sdk_dir: PathBuf = match std::env::var("FMSH_UKEY_SDK_DIR") {
        Ok(dir) => {
            let dir = PathBuf::from(dir);
            if dir.is_absolute() {
                dir
            } else {
                workspace_root.join(dir)
            }
        }
        Err(_) => panic!("FMSH_UKEY_SDK_DIR must be set"),
    };
    let sdk_dir = sdk_dir.canonicalize().unwrap_or_else(|e| {
        panic!(
            "SDK dir {} not found: {e} (set FMSH_UKEY_SDK_DIR)",
            sdk_dir.display()
        )
    });

    let lib_dir = sdk_dir.join("linux/lib");
    assert!(
        lib_dir.join("libfmsh_ukey_sdk.so").exists(),
        "libfmsh_ukey_sdk.so not found in {}",
        lib_dir.display()
    );

    println!("cargo:rerun-if-env-changed=FMSH_UKEY_SDK_DIR");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=fmsh_ukey_sdk");
    // RUNPATH on the final binary/cdylib resolves libfmsh_ukey_sdk.so itself
    // plus its NEEDED libcrypto.so.1.1 (both live in linux/lib).
    //
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    // NOTE: link flags only propagate to crates whose build.rs emits them,
    // so every cdylib crate that (transitively) links the SDK must emit the
    // same rpath itself — see bindings/*/build.rs.

    // Expose for tests/docs.
    println!(
        "cargo:rustc-env=FMSH_UKEY_SDK_DIR_RESOLVED={}",
        sdk_dir.display()
    );
    let _ = Path::new("");
}
