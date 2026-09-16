use std::env;

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("linux")
        || env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("x86_64")
        || env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("gnu")
    {
        return;
    }
    let lib_dir = match env::var("DEP_FMSH_UKEY_SDK_LIB_DIR") {
        Ok(dir) => dir,
        Err(_) => panic!("DEP_FMSH_UKEY_SDK_LIB_DIR missing; fmsh-ukey-core must be a direct dependency"),
    };
    println!("cargo:rustc-link-search=native={lib_dir}");
    println!("cargo:rustc-link-lib=dylib=fmsh_ukey_sdk");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{lib_dir}");
    println!("cargo:rerun-if-env-changed=DEP_FMSH_UKEY_SDK_LIB_DIR");
}
