//! Two-phase backend:
//! phase 1 — UKey unwraps one symmetric key (single hardware call);
//! phase 2 — every file is AES-256-GCM in software.

use std::sync::Arc;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use openssl::rand::rand_bytes;
use openssl::symm::Cipher as OsslCipher;
use openssl::symm::Crypter;
use openssl::symm::Mode;
use parking_lot::RwLock;
use zeroize::Zeroizing;

use crate::cipher::Cipher;
use crate::cipher::KeyWrap;
use crate::cipher::GCM_IV_LEN;
use crate::cipher::GCM_TAG_LEN;

/// Magic prefix of every `ukey-two-phase` data file.
pub const TWO_PHASE_MAGIC: &[u8; 8] = b"FMSH2PH1";

/// Size of the phase-2 key (AES-256-GCM). The key is wrapped inside a CMS
/// envelope by the UKey in phase 1; unwrapping must yield exactly this many
/// bytes.
pub const TWO_PHASE_KEY_LEN: usize = 32;

/// Phase 1 unwraps one symmetric key through the UKey and keeps it in process
/// memory; phase 2 encrypts/decrypts every file with software AES-256-GCM.
///
/// Data-file format (fixed sizes):
/// `[magic "FMSH2PH1" 8][iv 12][auth_tag 16][ciphertext]`.
///
/// The wrapped key file is an ordinary CMS envelope containing exactly
/// [`TWO_PHASE_KEY_LEN`] bytes.
pub struct UkeyTwoPhaseCipher {
    key_wrap: Arc<dyn KeyWrap>,
    key: RwLock<Option<Zeroizing<[u8; TWO_PHASE_KEY_LEN]>>>,
}

impl UkeyTwoPhaseCipher {
    pub fn new(key_wrap: Arc<dyn KeyWrap>) -> Self {
        Self {
            key_wrap,
            key: RwLock::new(None),
        }
    }

    /// Whether the phase-2 key is loaded (or generated) in memory.
    pub fn key_ready(&self) -> bool {
        self.key.read().is_some()
    }

    /// Phase 1: unwrap a wrapped key through the key wrap backend and keep it
    /// in memory. This is the single UKey hardware call of a decrypt run.
    pub fn unwrap_key(&self, wrapped: &[u8]) -> Result<()> {
        let plaintext = self
            .key_wrap
            .unwrap(wrapped)
            .context("unwrapping phase-2 key")?;
        if plaintext.len() != TWO_PHASE_KEY_LEN {
            bail!(
                "key envelope must contain {TWO_PHASE_KEY_LEN} bytes, got {}",
                plaintext.len()
            );
        }
        let mut key = Zeroizing::new([0u8; TWO_PHASE_KEY_LEN]);
        key.copy_from_slice(&plaintext);
        *self.key.write() = Some(key);
        Ok(())
    }

    /// Wrap the current phase-2 key (generating one if needed) into an
    /// envelope that can later be unwrapped with [`Self::unwrap_key`].
    pub fn wrap_key(&self) -> Result<Vec<u8>> {
        let key = self.ensure_key()?;
        self.key_wrap.wrap(&key[..])
    }

    fn ensure_key(&self) -> Result<Zeroizing<[u8; TWO_PHASE_KEY_LEN]>> {
        {
            let guard = self.key.read();
            if let Some(key) = guard.as_ref() {
                return Ok(key.clone());
            }
        }
        let mut key = Zeroizing::new([0u8; TWO_PHASE_KEY_LEN]);
        rand_bytes(&mut *key)?;
        let mut guard = self.key.write();
        if let Some(existing) = guard.as_ref() {
            Ok(existing.clone())
        } else {
            *guard = Some(key.clone());
            Ok(key)
        }
    }
}

impl Cipher for UkeyTwoPhaseCipher {
    fn mode(&self) -> &'static str {
        "ukey-two-phase"
    }

    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let key = self.ensure_key()?;
        let mut iv = [0u8; GCM_IV_LEN];
        rand_bytes(&mut iv)?;
        let mut crypter = Crypter::new(
            OsslCipher::aes_256_gcm(),
            Mode::Encrypt,
            &key[..],
            Some(&iv),
        )?;
        let mut encrypted = vec![0u8; plaintext.len() + GCM_TAG_LEN];
        let mut n = crypter.update(plaintext, &mut encrypted)?;
        n += crypter.finalize(&mut encrypted[n..])?;
        let mut tag = [0u8; GCM_TAG_LEN];
        crypter.get_tag(&mut tag)?;
        encrypted.truncate(n);

        let mut out =
            Vec::with_capacity(TWO_PHASE_MAGIC.len() + GCM_IV_LEN + GCM_TAG_LEN + encrypted.len());
        out.extend_from_slice(TWO_PHASE_MAGIC);
        out.extend_from_slice(&iv);
        out.extend_from_slice(&tag);
        out.extend_from_slice(&encrypted);
        Ok(out)
    }

    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let key = self
            .key
            .read()
            .clone()
            .ok_or_else(|| anyhow!("ukey-two-phase mode: phase-2 key not loaded"))?;

        let header_len = TWO_PHASE_MAGIC.len() + GCM_IV_LEN + GCM_TAG_LEN;
        if ciphertext.len() < header_len {
            bail!("ukey-two-phase file too short");
        }
        if &ciphertext[..TWO_PHASE_MAGIC.len()] != TWO_PHASE_MAGIC {
            bail!("not a ukey-two-phase encrypted file (bad magic)");
        }
        let (iv, rest) = ciphertext[TWO_PHASE_MAGIC.len()..].split_at(GCM_IV_LEN);
        let (tag, encrypted) = rest.split_at(GCM_TAG_LEN);

        let mut crypter =
            Crypter::new(OsslCipher::aes_256_gcm(), Mode::Decrypt, &key[..], Some(iv))?;
        crypter.set_tag(tag)?;
        let mut plaintext = vec![0u8; encrypted.len()];
        let mut n = crypter.update(encrypted, &mut plaintext)?;
        n += crypter.finalize(&mut plaintext[n..])?;
        plaintext.truncate(n);
        Ok(plaintext)
    }
}
