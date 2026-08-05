//! Pluggable byte-level encryption backends.
//!
//! Module layout:
//! - [`noop`] — identity transform
//! - [`local`] — X25519 digital envelope
//! - [`ukey`] — CMS SM2/SM4 envelope via the FMSH UKey SDK
//! - [`two_phase`] — phase 1: UKey unwraps one key; phase 2: software AES-256-GCM

use anyhow::Result;

mod local;
mod noop;
mod two_phase;
mod ukey;

pub use local::LocalCipher;
pub use noop::NoopCipher;
pub use two_phase::UkeyTwoPhaseCipher;
pub use two_phase::TWO_PHASE_MAGIC;
pub use ukey::UkeyCipher;
pub use ukey::UkeyKeyWrap;

/// Shared GCM layout constants (12-byte IV, 16-byte auth tag).
pub(crate) const GCM_IV_LEN: usize = 12;
pub(crate) const GCM_TAG_LEN: usize = 16;

/// Abstraction over byte-level encryption/decryption. Every backend is
/// `Send + Sync`; `ukey-two-phase` is the only one designed for concurrent
/// decrypts, because it keeps the symmetric key in memory after one UKey call.
pub trait Cipher: Send + Sync {
    /// Stable mode name used by the CLI (`noop`, `local`, `ukey`, `ukey-two-phase`).
    fn mode(&self) -> &'static str;

    /// Encrypt `plaintext` into an envelope.
    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>>;

    /// Decrypt an envelope back to plaintext.
    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>>;
}

/// Key-wrapping primitive used by [`UkeyTwoPhaseCipher`].
///
/// The real implementation is [`UkeyKeyWrap`] (CMS envelope through the SDK).
/// The trait exists so the two-phase logic is testable without a UKey and can be
/// swapped for another key-escrow mechanism.
pub trait KeyWrap: Send + Sync {
    /// Wrap a plaintext key into an envelope.
    fn wrap(&self, plaintext: &[u8]) -> Result<Vec<u8>>;

    /// Unwrap an envelope back to the plaintext key.
    fn unwrap(&self, envelope: &[u8]) -> Result<Vec<u8>>;
}
