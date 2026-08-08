fn main() {
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
