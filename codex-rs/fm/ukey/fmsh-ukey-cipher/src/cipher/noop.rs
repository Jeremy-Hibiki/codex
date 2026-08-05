//! NOOP backend: identity transform.

use anyhow::Result;

use crate::cipher::Cipher;

/// Identity cipher. Bytes pass through unchanged; at the file layer this
/// means decrypt output is the input minus `.enc` (and encrypt appends it).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopCipher;

impl Cipher for NoopCipher {
    fn mode(&self) -> &'static str {
        "noop"
    }

    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        Ok(plaintext.to_vec())
    }

    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        Ok(ciphertext.to_vec())
    }
}
