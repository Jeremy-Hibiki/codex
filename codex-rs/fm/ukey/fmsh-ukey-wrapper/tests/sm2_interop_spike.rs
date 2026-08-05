//! SM2 CMS interop spike (Task 1.1) — OpenSSL **CLI** path.
//!
//! Gates the entire `fmsh-ukey-enc` crate: proves that an SM2 CMS envelope
//! produced by `fmsh_ukey_wrapper::Ukey::encrypt(user_cert, payload)` can be
//! decrypted by the system OpenSSL 3.x CLI (`openssl cms -decrypt`) using the
//! matching SM2 private key. The spike is **self-contained**: it generates a
//! fresh SM2 key pair + self-signed cert via the `openssl` CLI in a tempdir,
//! so it does not depend on any sibling-directory key material.
//!
//! ## Why the CLI, not the `openssl` crate
//!
//! The FMSH SDK ships `libcrypto.so` (1.1) in its `linux/lib`, and core's
//! `build.rs` adds that directory to the link search path so the SDK resolves
//! its own `libcrypto.so.1.1` dependency. The Rust `openssl` crate
//! (`openssl-sys`) needs OpenSSL 3.x symbols (`ERR_get_error_all`, …) for SM2
//! CMS; with the SDK's 1.1 libcrypto on the search path first, the link fails
//! with `undefined symbol: ERR_get_error_all`. Vendoring OpenSSL 3.x would
//! bloat every binary that links `fmsh-ukey-enc`; the design (D3 + task 1.3)
//! explicitly sanctions shelling out to the `openssl` CLI as the inner CMS
//! layer. The playground spike already proved CLI↔FMSH interop.
//!
//! ## Hardware / skip behaviour
//!
//! The encrypt direction is offline (provider-only); device/PIN not needed.
//! Without `FMSH_UKEY_PROVIDER` the SDK can't init and the test skips. No
//! wrong-PIN case — GM3000 retry exhaustion locks the device.

#![cfg(test)]

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use fmsh_ukey_wrapper::Ukey;

struct Tmp {
    dir: tempfile::TempDir,
}
impl Tmp {
    fn new() -> Self {
        Self {
            dir: tempfile::TempDir::new().expect("tempdir"),
        }
    }
    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }
    fn write(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let p = self.path(name);
        fs::write(&p, bytes).expect("write");
        p
    }
}

/// Generate a fresh SM2 keypair + self-signed cert via the openssl CLI.
/// Returns (cert_pem_path, key_pem_path).
fn gen_sm2_keypair_cert_cli(tmp: &Tmp) -> (PathBuf, PathBuf) {
    let key_path = tmp.path("sm2-key.pem");
    let cert_path = tmp.path("cert.pem");

    // SM2 private key.
    let s = Command::new("openssl")
        .args(["ecparam", "-name", "SM2", "-genkey", "-noout"])
        .arg("-out")
        .arg(&key_path)
        .stderr(Stdio::inherit())
        .status()
        .expect("openssl ecparam");
    assert!(s.success(), "openssl ecparam SM2 genkey failed");

    // Self-signed cert. `openssl req -x509` with SM3 digest.
    let s = Command::new("openssl")
        .args(["req", "-x509", "-new", "-key"])
        .arg(&key_path)
        .args(["-out"])
        .arg(&cert_path)
        .args(["-days", "1", "-SM3", "-subj", "/CN=fmsh-ukey-enc-spike"])
        .stderr(Stdio::inherit())
        .status()
        .expect("openssl req");
    assert!(s.success(), "openssl req -x509 failed");

    (cert_path, key_path)
}

/// Confirm `openssl cms -decrypt` can open an FMSH-produced SM2 envelope.
#[test]
fn fmsh_encrypt_openssl_cli_decrypt_roundtrip() {
    // OpenSSL 3.x gate.
    let vstr = Command::new("openssl")
        .arg("version")
        .output()
        .expect("openssl version");
    assert!(
        String::from_utf8_lossy(&vstr.stdout).contains("OpenSSL 3"),
        "this spike requires OpenSSL 3.x for SM2 CMS"
    );

    // Encryption direction is offline (provider-only); skip if SDK can't init.
    let ukey = match Ukey::initialize() {
        Ok(u) => u,
        Err(e) => {
            eprintln!(
                "[skip] sm2_interop_spike: FMSH_UKEY_PROVIDER not set / SDK init failed: {}",
                e
            );
            return;
        }
    };

    let tmp = Tmp::new();
    let (cert_path, key_path) = gen_sm2_keypair_cert_cli(&tmp);
    let cert_der = {
        // FMSH accepts PEM or DER; emit DER for the core API.
        let out = Command::new("openssl")
            .args(["x509", "-in"])
            .arg(&cert_path)
            .args(["-outform", "DER"])
            .output()
            .expect("openssl x509 -outform DER");
        assert!(out.status.success(), "openssl x509 DER conversion failed");
        out.stdout
    };

    let payload = b"two-factor spike payload \x00 binary safe";

    // UKey → SM2 CMS envelope for the freshly-generated user cert.
    let envelope = ukey.encrypt(&cert_der, payload).expect("Ukey::encrypt");

    // Write envelope to a tempfile (CMS bytes, not sensitive on their own —
    // they're ciphertext). The private key already lives in the tempdir.
    let env_path = tmp.write("envelope.p7m", &envelope);

    // `openssl cms -decrypt -in envelope -inkey key -recip cert`.
    // -binary preserves bytes; -inform DER because FMSH emits DER CMS.
    let dec = Command::new("openssl")
        .args(["cms", "-decrypt", "-binary"])
        .args(["-inform", "DER"])
        .arg("-in")
        .arg(&env_path)
        .arg("-inkey")
        .arg(&key_path)
        .arg("-recip")
        .arg(&cert_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .output()
        .expect("openssl cms -decrypt");
    assert!(
        dec.status.success(),
        "openssl cms -decrypt failed: {}",
        String::from_utf8_lossy(&dec.stderr)
    );
    assert_eq!(dec.stdout, payload);

    // Do NOT call Ukey::finalize() here: the test harness may run other tests
    // in the same process that need an initialized SDK. The process-exit path
    // handles teardown.
    let _ = ukey;
}

/// OpenSSL version sanity (the CMS SM2 path needs ≥3.0).
#[test]
fn openssl_cli_version_is_3x() {
    let out = Command::new("openssl")
        .arg("version")
        .output()
        .expect("openssl version");
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("OpenSSL 3"), "expected OpenSSL 3.x, got: {s}");
}
