//! Local X25519 digital envelope backend.

use std::path::Path;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use openssl::derive::Deriver;
use openssl::kdf::hkdf;
use openssl::kdf::HkdfMode;
use openssl::md::Md;
use openssl::pkey::Id;
use openssl::pkey::PKey;
use openssl::rand::rand_bytes;
use openssl::symm::Cipher as OsslCipher;
use openssl::symm::Crypter;
use openssl::symm::Mode;

use crate::cipher::Cipher;
use crate::cipher::GCM_IV_LEN;
use crate::cipher::GCM_TAG_LEN;

/// HKDF salt/info for the local X25519 envelope. Intentionally identical to
/// `fmsh-ukey-enc`'s local mode so envelopes interoperate across this crate,
/// the skill CLI, and the Node/Python/Bun bindings.
const HKDF_SALT: &[u8] = b"fmsh-ukey-enc";
const HKDF_INFO: &[u8] = b"skill-envelope-v1";

/// X25519 digital envelope backend.
///
/// An ephemeral X25519 key pair is generated per message; the ECDH shared
/// secret with the recipient's static X25519 public key is expanded via
/// HKDF-SHA256 into an AES-256-GCM key. Envelope layout:
/// `[ephemeral_pub 32][iv 12][auth_tag 16][ciphertext]`.
///
/// The same key files (PEM `PUBLIC KEY` / `PRIVATE KEY`) work with the
/// `fmsh-ukey-enc` local mode.
pub struct LocalCipher {
    pub_key: Option<PKey<openssl::pkey::Public>>,
    priv_key: Option<PKey<openssl::pkey::Private>>,
}

impl LocalCipher {
    /// Load only the public key — usable for encryption.
    pub fn from_pub_file(path: &Path) -> Result<Self> {
        let pem = std::fs::read(path)
            .with_context(|| format!("reading public key {}", path.display()))?;
        let pkey = PKey::public_key_from_pem(&pem)
            .with_context(|| format!("parsing public key {}", path.display()))?;
        if pkey.id() != Id::X25519 {
            bail!("{} is not an X25519 public key", path.display());
        }
        Ok(Self {
            pub_key: Some(pkey),
            priv_key: None,
        })
    }

    /// Load only the private key — usable for decryption.
    pub fn from_priv_file(path: &Path) -> Result<Self> {
        let pem = std::fs::read(path)
            .with_context(|| format!("reading private key {}", path.display()))?;
        let pkey = PKey::private_key_from_pem(&pem)
            .with_context(|| format!("parsing private key {}", path.display()))?;
        if pkey.id() != Id::X25519 {
            bail!("{} is not an X25519 private key", path.display());
        }
        Ok(Self {
            pub_key: None,
            priv_key: Some(pkey),
        })
    }

    /// Load both halves.
    pub fn from_key_files(pub_path: &Path, priv_path: &Path) -> Result<Self> {
        let with_pub = Self::from_pub_file(pub_path)?;
        let with_priv = Self::from_priv_file(priv_path)?;
        Ok(Self {
            pub_key: with_pub.pub_key,
            priv_key: with_priv.priv_key,
        })
    }

    /// Generate a fresh X25519 key pair, persist both halves, and return a
    /// cipher that can both encrypt and decrypt.
    pub fn generate_to_files(pub_path: &Path, priv_path: &Path) -> Result<Self> {
        let key = PKey::generate_x25519()?;
        std::fs::write(pub_path, key.public_key_to_pem()?)?;
        std::fs::write(priv_path, key.private_key_to_pem_pkcs8()?)?;
        Self::from_key_files(pub_path, priv_path)
    }
}

impl Cipher for LocalCipher {
    fn mode(&self) -> &'static str {
        "local"
    }

    fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>> {
        let pub_key = self
            .pub_key
            .as_ref()
            .ok_or_else(|| anyhow!("local mode: encryption requires --local-pubkey"))?;

        // Ephemeral X25519 key pair per message.
        let ephemeral = PKey::generate_x25519()?;
        let eph_pub = ephemeral.raw_public_key()?;
        debug_assert_eq!(eph_pub.len(), 32);

        // ECDH shared secret with the recipient's static public key.
        let mut deriver = Deriver::new(&ephemeral)?;
        deriver.set_peer(pub_key)?;
        let shared = deriver.derive_to_vec()?;

        // HKDF-SHA256 → AES-256-GCM key.
        let mut aes_key = [0u8; 32];
        hkdf(
            Md::sha256(),
            &shared,
            Some(HKDF_SALT),
            Some(HKDF_INFO),
            HkdfMode::ExtractAndExpand,
            None,
            &mut aes_key,
        )?;

        // AES-256-GCM encrypt with random 12-byte IV.
        let mut iv = [0u8; GCM_IV_LEN];
        rand_bytes(&mut iv)?;
        let mut crypter = Crypter::new(
            OsslCipher::aes_256_gcm(),
            Mode::Encrypt,
            &aes_key,
            Some(&iv),
        )?;
        let mut ciphertext = vec![0u8; plaintext.len() + GCM_TAG_LEN];
        let mut n = crypter.update(plaintext, &mut ciphertext)?;
        n += crypter.finalize(&mut ciphertext[n..])?;
        let mut tag = [0u8; GCM_TAG_LEN];
        crypter.get_tag(&mut tag)?;
        ciphertext.truncate(n);

        // Envelope: [eph_pub 32][iv 12][tag 16][ciphertext].
        let mut envelope = Vec::with_capacity(32 + GCM_IV_LEN + GCM_TAG_LEN + ciphertext.len());
        envelope.extend_from_slice(&eph_pub);
        envelope.extend_from_slice(&iv);
        envelope.extend_from_slice(&tag);
        envelope.extend_from_slice(&ciphertext);
        Ok(envelope)
    }

    fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>> {
        let priv_key = self
            .priv_key
            .as_ref()
            .ok_or_else(|| anyhow!("local mode: decryption requires --local-privkey"))?;

        if ciphertext.len() < 32 + GCM_IV_LEN + GCM_TAG_LEN {
            bail!("local envelope too short");
        }
        let (eph_pub, rest) = ciphertext.split_at(32);
        let (iv, rest) = rest.split_at(GCM_IV_LEN);
        let (tag, encrypted) = rest.split_at(GCM_TAG_LEN);

        // Reconstruct the ephemeral public key from raw bytes.
        let eph_pub_key = PKey::public_key_from_raw_bytes(eph_pub, Id::X25519)?;

        // ECDH shared secret with our static private key.
        let mut deriver = Deriver::new(priv_key)?;
        deriver.set_peer(&eph_pub_key)?;
        let shared = deriver.derive_to_vec()?;

        // HKDF-SHA256 → AES-256-GCM key.
        let mut aes_key = [0u8; 32];
        hkdf(
            Md::sha256(),
            &shared,
            Some(HKDF_SALT),
            Some(HKDF_INFO),
            HkdfMode::ExtractAndExpand,
            None,
            &mut aes_key,
        )?;

        // AES-256-GCM decrypt.
        let mut crypter =
            Crypter::new(OsslCipher::aes_256_gcm(), Mode::Decrypt, &aes_key, Some(iv))?;
        crypter.set_tag(tag)?;
        let mut plaintext = vec![0u8; encrypted.len()];
        let mut n = crypter.update(encrypted, &mut plaintext)?;
        n += crypter.finalize(&mut plaintext[n..])?;
        plaintext.truncate(n);
        Ok(plaintext)
    }
}
