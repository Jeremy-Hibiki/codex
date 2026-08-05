//! fmsh-ukey-cipher — pluggable file encryption/decryption library and CLI.
//!
//! Everything goes through the [`Cipher`] abstraction so callers and the CLI
//! can switch backends without touching crypto logic:
//!
//! | mode          | backend |
//! |---------------|---------|
//! | `noop`        | identity transform; decrypt output is the input minus `.enc` |
//! | `local`       | X25519 digital envelope (ECDH + HKDF-SHA256 + AES-256-GCM) |
//! | `ukey`        | CMS SM2/SM4 envelope through the FMSH UKey SDK |
//! | `ukey-two-phase`| phase 1: UKey unwraps one AES-256-GCM key; phase 2: every other file is decrypted in software |
//!
//! The `ukey-two-phase` mode exists because each UKey decrypt call is a slow
//! hardware roundtrip and the SDK serializes device access. The first call
//! (phase 1) unwraps a wrapped symmetric key from a key envelope; the key
//! stays in process memory and all remaining `.enc` files (phase 2) are decrypted with
//! AES-256-GCM without touching the device.

use std::path::Path;
use std::path::PathBuf;

mod cipher;

pub use cipher::Cipher;
pub use cipher::KeyWrap;
pub use cipher::LocalCipher;
pub use cipher::NoopCipher;
pub use cipher::UkeyCipher;
pub use cipher::UkeyKeyWrap;
pub use cipher::UkeyTwoPhaseCipher;
pub use cipher::TWO_PHASE_MAGIC;

/// Append `.enc` to a path: `a/b.txt` → `a/b.txt.enc`.
pub fn append_enc(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".enc");
    PathBuf::from(name)
}

/// Strip a trailing `.enc` suffix: `a/b.txt.enc` → `a/b.txt`.
/// Returns `None` when the path does not end in `.enc`.
pub fn strip_enc(path: &Path) -> Option<PathBuf> {
    let name = path.as_os_str().to_string_lossy();
    name.strip_suffix(".enc").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_and_strip_enc() {
        let src = Path::new("a/b.txt");
        let enc = append_enc(src);
        assert_eq!(enc, PathBuf::from("a/b.txt.enc"));
        assert_eq!(strip_enc(&enc), Some(src.to_path_buf()));
        assert_eq!(strip_enc(src), None);
    }
}
