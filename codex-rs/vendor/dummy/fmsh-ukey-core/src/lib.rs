//! Placeholder for the internal `fmsh-ukey-core` crate of `fmsh-ukey-lib`
//! (internal GitLab 192.168.131.126:8089). See the sibling placeholder
//! crates and `release/use-real-fm-deps.sh` for the switch-back workflow.
//!
//! The stub mirrors the real crate's public API:
//!
//! - The **software envelope is implemented for real**: X25519 ephemeral
//!   ECDH + HKDF-SHA256 + AES-256-GCM, with the upstream `local`-mode
//!   envelope layout `[eph_pub 32][iv 12][tag 16][ciphertext]`. Round trips
//!   are self-consistent, so the `fm-encrypted-skills` tests that exercise
//!   the software pipeline run against this stub.
//! - The **UKey hardware backends fail closed**: `UkeyCipher::new` and
//!   `UkeyKeyWrap::new` return an error, surfacing as
//!   `EnvelopeError::HardwareKeyRequired` upstream.
//! - **SM2/SM4 CMS** (`SoftwareAlgorithm::Sm2Sm4Cbc`) is not implementable
//!   offline and returns a clear error.
//! - `UkeyTwoPhaseCipher` is fully functional: it manages an in-memory
//!   AES-256-GCM key wrapped/unwraped through the pluggable [`KeyWrap`]
//!   trait, using the upstream `FMSH2PH1` data format
//!   `[magic 8][iv 12][tag 16][ciphertext]`.

use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use aes_gcm::Aes256Gcm;
use aes_gcm::Nonce;
use aes_gcm::aead::Aead;
use aes_gcm::aead::KeyInit;
use hkdf::Hkdf;
use sha2::Sha256;

const SOFTWARE_PRIV_HEADER: &str = "-----BEGIN GREVO STUB SOFTWARE PRIVATE KEY-----";
const SOFTWARE_PRIV_FOOTER: &str = "-----END GREVO STUB SOFTWARE PRIVATE KEY-----";
const SOFTWARE_PUB_HEADER: &str = "-----BEGIN GREVO STUB SOFTWARE PUBLIC KEY-----";
const SOFTWARE_PUB_FOOTER: &str = "-----END GREVO STUB SOFTWARE PUBLIC KEY-----";
const HKDF_INFO: &[u8] = b"grevo-stub-software-envelope-v1";
const EPH_PUB_LEN: usize = 32;
const IV_LEN: usize = 12;
const TAG_LEN: usize = 16;
const TWO_PHASE_MAGIC: &[u8; 8] = b"FMSH2PH1";

fn fill_random(bytes: &mut [u8]) -> anyhow::Result<()> {
    getrandom::fill(bytes).map_err(|err| anyhow::anyhow!("OsRng unavailable: {err}"))
}

fn hkdf_key(shared: &[u8]) -> anyhow::Result<[u8; 32]> {
    let hkdf = Hkdf::<Sha256>::new(None, shared);
    let mut key = [0u8; 32];
    hkdf.expand(HKDF_INFO, &mut key)
        .map_err(|err| anyhow::anyhow!("HKDF expand failed: {err}"))?;
    Ok(key)
}

fn aes_gcm_seal(key: &[u8; 32], iv: &[u8; IV_LEN], plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key.as_slice())
        .map_err(|err| anyhow::anyhow!("invalid AES key: {err}"))?;
    let nonce = Nonce::from_slice(iv);
    cipher
        .encrypt(nonce, plaintext)
        .map_err(|err| anyhow::anyhow!("AES-256-GCM encrypt failed: {err}"))
}

fn aes_gcm_open(
    key: &[u8; 32],
    iv: &[u8; IV_LEN],
    ciphertext_and_tag: &[u8],
) -> anyhow::Result<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(key.as_slice())
        .map_err(|err| anyhow::anyhow!("invalid AES key: {err}"))?;
    let nonce = Nonce::from_slice(iv);
    cipher
        .decrypt(nonce, ciphertext_and_tag)
        .map_err(|err| anyhow::anyhow!("AES-256-GCM decrypt failed: {err}"))
}

/// Digital-envelope cipher over skill package bytes.
pub trait Cipher: Send + Sync {
    /// Upstream mode name (`local` / `ukey` / `ukey-two-phase`).
    fn mode(&self) -> &'static str;
    fn encrypt(&self, plaintext: &[u8]) -> anyhow::Result<Vec<u8>>;
    fn decrypt(&self, ciphertext: &[u8]) -> anyhow::Result<Vec<u8>>;
}

/// Pluggable key wrap for the two-phase envelope. The real UKey
/// implementation wraps through the hardware; tests substitute a software
/// wrap so the two-phase pipeline runs without a key attached.
pub trait KeyWrap: Send + Sync {
    fn wrap(&self, plaintext: &[u8]) -> anyhow::Result<Vec<u8>>;
    fn unwrap(&self, envelope: &[u8]) -> anyhow::Result<Vec<u8>>;
}

/// Selectable algorithm for the software envelope backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftwareAlgorithm {
    /// X25519 + HKDF-SHA256 + AES-256-GCM (implemented by this stub).
    HpkeX25519Aes256Gcm,
    /// CMS EnvelopedData (SM2 key transport + SM4-CBC, GM/T 0010).
    /// Not implementable offline; every entry point errors out.
    Sm2Sm4Cbc,
}

/// Software (no hardware) envelope cipher.
pub struct SoftwareCipher {
    algorithm: SoftwareAlgorithm,
    private: x25519_dalek::StaticSecret,
    public: x25519_dalek::PublicKey,
}

impl SoftwareCipher {
    /// Generate a fresh key pair and write both PEM files.
    pub fn generate_to_files(
        pub_path: &Path,
        priv_path: &Path,
        algorithm: SoftwareAlgorithm,
    ) -> anyhow::Result<Self> {
        Self::ensure_supported(algorithm)?;
        let mut secret = [0u8; 32];
        fill_random(&mut secret)?;
        let private = x25519_dalek::StaticSecret::from(secret);
        let public = x25519_dalek::PublicKey::from(&private);
        fs::write(priv_path, encode_pem(SOFTWARE_PRIV_HEADER, SOFTWARE_PRIV_FOOTER, &key_blob(algorithm, private.as_bytes())))?;
        fs::write(pub_path, encode_pem(SOFTWARE_PUB_HEADER, SOFTWARE_PUB_FOOTER, &key_blob(algorithm, public.as_bytes())))?;
        Ok(Self {
            algorithm,
            private,
            public,
        })
    }

    /// Load the private key PEM written by [`generate_to_files`].
    pub fn from_priv_file(path: &Path, algorithm: SoftwareAlgorithm) -> anyhow::Result<Self> {
        Self::ensure_supported(algorithm)?;
        let pem = fs::read_to_string(path)
            .map_err(|err| anyhow::anyhow!("reading software private key {}: {err}", path.display()))?;
        let blob = decode_pem(&pem, SOFTWARE_PRIV_HEADER, SOFTWARE_PRIV_FOOTER)
            .ok_or_else(|| anyhow::anyhow!("invalid software private key PEM {}", path.display()))?;
        let (file_algorithm, secret) = parse_key_blob(&blob)
            .ok_or_else(|| anyhow::anyhow!("malformed software private key {}", path.display()))?;
        if file_algorithm != algorithm {
            anyhow::bail!(
                "software private key algorithm mismatch: file is {file_algorithm:?}, requested {algorithm:?}"
            );
        }
        let private = x25519_dalek::StaticSecret::from(secret);
        let public = x25519_dalek::PublicKey::from(&private);
        Ok(Self {
            algorithm,
            private,
            public,
        })
    }

    fn ensure_supported(algorithm: SoftwareAlgorithm) -> anyhow::Result<()> {
        match algorithm {
            SoftwareAlgorithm::HpkeX25519Aes256Gcm => Ok(()),
            SoftwareAlgorithm::Sm2Sm4Cbc => anyhow::bail!(
                "SoftwareAlgorithm::Sm2Sm4Cbc (CMS SM2/SM4) is not supported by the offline \
                 fmsh-ukey-core placeholder; run release/use-real-fm-deps.sh for the real envelope"
            ),
        }
    }
}

impl Cipher for SoftwareCipher {
    fn mode(&self) -> &'static str {
        "local"
    }

    fn encrypt(&self, plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
        Self::ensure_supported(self.algorithm)?;
        let ephemeral = x25519_dalek::StaticSecret::from(rand_32()?);
        let ephemeral_public = x25519_dalek::PublicKey::from(&ephemeral);
        let shared = ephemeral.diffie_hellman(&self.public);
        let key = hkdf_key(shared.as_bytes())?;
        let mut iv = [0u8; IV_LEN];
        fill_random(&mut iv)?;
        let ct_and_tag = aes_gcm_seal(&key, &iv, plaintext)?;
        let (ct, tag) = ct_and_tag.split_at(ct_and_tag.len() - TAG_LEN);
        // Upstream local-mode layout: [eph_pub 32][iv 12][tag 16][ct].
        let mut envelope = Vec::with_capacity(EPH_PUB_LEN + IV_LEN + TAG_LEN + ct.len());
        envelope.extend_from_slice(ephemeral_public.as_bytes());
        envelope.extend_from_slice(&iv);
        envelope.extend_from_slice(tag);
        envelope.extend_from_slice(ct);
        Ok(envelope)
    }

    fn decrypt(&self, envelope: &[u8]) -> anyhow::Result<Vec<u8>> {
        Self::ensure_supported(self.algorithm)?;
        if envelope.len() < EPH_PUB_LEN + IV_LEN + TAG_LEN {
            anyhow::bail!("software envelope too short ({} bytes)", envelope.len());
        }
        let ephemeral_public: [u8; EPH_PUB_LEN] = envelope[..EPH_PUB_LEN].try_into()?;
        let iv: [u8; IV_LEN] = envelope[EPH_PUB_LEN..EPH_PUB_LEN + IV_LEN].try_into()?;
        let tag = &envelope[EPH_PUB_LEN + IV_LEN..EPH_PUB_LEN + IV_LEN + TAG_LEN];
        let ct = &envelope[EPH_PUB_LEN + IV_LEN + TAG_LEN..];
        let ephemeral_public = x25519_dalek::PublicKey::from(ephemeral_public);
        let shared = self.private.diffie_hellman(&ephemeral_public);
        let key = hkdf_key(shared.as_bytes())?;
        let mut ct_and_tag = Vec::with_capacity(ct.len() + TAG_LEN);
        ct_and_tag.extend_from_slice(ct);
        ct_and_tag.extend_from_slice(tag);
        aes_gcm_open(&key, &iv, &ct_and_tag)
    }
}

fn rand_32() -> anyhow::Result<[u8; 32]> {
    let mut bytes = [0u8; 32];
    fill_random(&mut bytes)?;
    Ok(bytes)
}

fn key_blob(algorithm: SoftwareAlgorithm, key_bytes: &[u8]) -> Vec<u8> {
    let alg = match algorithm {
        SoftwareAlgorithm::HpkeX25519Aes256Gcm => 1u8,
        SoftwareAlgorithm::Sm2Sm4Cbc => 2u8,
    };
    let mut blob = Vec::with_capacity(1 + key_bytes.len());
    blob.push(alg);
    blob.extend_from_slice(key_bytes);
    blob
}

fn parse_key_blob(blob: &[u8]) -> Option<(SoftwareAlgorithm, [u8; 32])> {
    if blob.len() != 33 {
        return None;
    }
    let algorithm = match blob[0] {
        1 => SoftwareAlgorithm::HpkeX25519Aes256Gcm,
        2 => SoftwareAlgorithm::Sm2Sm4Cbc,
        _ => return None,
    };
    let key: [u8; 32] = blob[1..].try_into().ok()?;
    Some((algorithm, key))
}

fn encode_pem(header: &str, footer: &str, blob: &[u8]) -> Vec<u8> {
    use base64::Engine;
    let encoded = base64::engine::general_purpose::STANDARD.encode(blob);
    format!("{header}\n{encoded}\n{footer}\n").into_bytes()
}

fn decode_pem(pem: &str, header: &str, footer: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    let body = pem.lines().find_map(|line| {
        let line = line.trim();
        (!line.is_empty() && line != header && line != footer).then_some(line)
    })?;
    base64::engine::general_purpose::STANDARD.decode(body).ok()
}

/// CMS SM2/SM4 envelope through the FMSH UKey SDK. The stub has no hardware
/// backend: construction always fails, which upstream surfaces as
/// `EnvelopeError::HardwareKeyRequired`.
pub struct UkeyCipher {
    _private: (),
}

impl UkeyCipher {
    pub fn new(
        _provider: Option<String>,
        _library: Option<String>,
        _config: Option<String>,
    ) -> anyhow::Result<Self> {
        anyhow::bail!("FMSH UKey hardware backend is unavailable in the offline placeholder");
    }
}

impl Cipher for UkeyCipher {
    fn mode(&self) -> &'static str {
        "ukey"
    }

    fn encrypt(&self, _plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
        anyhow::bail!("UkeyCipher stub has no hardware backend")
    }

    fn decrypt(&self, _ciphertext: &[u8]) -> anyhow::Result<Vec<u8>> {
        anyhow::bail!("UkeyCipher stub has no hardware backend")
    }
}

/// Hardware key wrap (phase one of the two-phase envelope). Fails closed in
/// the stub for the same reason as [`UkeyCipher`].
pub struct UkeyKeyWrap {
    _private: (),
}

impl UkeyKeyWrap {
    pub fn new(
        _provider: Option<String>,
        _library: Option<String>,
        _config: Option<String>,
    ) -> anyhow::Result<Self> {
        anyhow::bail!("FMSH UKey hardware key wrap is unavailable in the offline placeholder");
    }
}

impl KeyWrap for UkeyKeyWrap {
    fn wrap(&self, _plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
        anyhow::bail!("UkeyKeyWrap stub has no hardware backend")
    }

    fn unwrap(&self, _envelope: &[u8]) -> anyhow::Result<Vec<u8>> {
        anyhow::bail!("UkeyKeyWrap stub has no hardware backend")
    }
}

/// Two-phase envelope: an in-memory AES-256-GCM key wrapped/unwraped through
/// a pluggable [`KeyWrap`]. Functional in the stub; only the *wrap* step
/// decides whether a hardware key is required.
pub struct UkeyTwoPhaseCipher {
    key_wrap: Arc<dyn KeyWrap>,
    key: Mutex<Option<[u8; 32]>>,
}

impl UkeyTwoPhaseCipher {
    pub fn new(key_wrap: Arc<dyn KeyWrap>) -> Self {
        Self {
            key_wrap,
            key: Mutex::new(Some(
                rand_32().expect("OsRng must be available to create a two-phase cipher"),
            )),
        }
    }

    /// Export the in-memory data key as a wrapped key envelope.
    pub fn wrap_key(&self) -> anyhow::Result<Vec<u8>> {
        let key = self
            .key
            .lock()
            .map_err(|_| anyhow::anyhow!("two-phase key lock poisoned"))?
            .ok_or_else(|| anyhow::anyhow!("two-phase data key is not available"))?;
        self.key_wrap.wrap(&key)
    }

    /// Install a data key from a wrapped key envelope (phase one).
    pub fn unwrap_key(&self, envelope: &[u8]) -> anyhow::Result<()> {
        let raw = self.key_wrap.unwrap(envelope)?;
        let key: [u8; 32] = raw
            .try_into()
            .map_err(|raw: Vec<u8>| anyhow::anyhow!("wrapped key is {} bytes, want 32", raw.len()))?;
        *self
            .key
            .lock()
            .map_err(|_| anyhow::anyhow!("two-phase key lock poisoned"))? = Some(key);
        Ok(())
    }
}

impl Cipher for UkeyTwoPhaseCipher {
    fn mode(&self) -> &'static str {
        "ukey-two-phase"
    }

    fn encrypt(&self, plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
        let key = *self
            .key
            .lock()
            .map_err(|_| anyhow::anyhow!("two-phase key lock poisoned"))?
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("two-phase data key is not unwrapped"))?;
        let mut iv = [0u8; IV_LEN];
        fill_random(&mut iv)?;
        let ct_and_tag = aes_gcm_seal(&key, &iv, plaintext)?;
        let (ct, tag) = ct_and_tag.split_at(ct_and_tag.len() - TAG_LEN);
        // Upstream format: [magic "FMSH2PH1" 8][iv 12][tag 16][ciphertext].
        let mut envelope = Vec::with_capacity(8 + IV_LEN + TAG_LEN + ct.len());
        envelope.extend_from_slice(TWO_PHASE_MAGIC);
        envelope.extend_from_slice(&iv);
        envelope.extend_from_slice(tag);
        envelope.extend_from_slice(ct);
        Ok(envelope)
    }

    fn decrypt(&self, envelope: &[u8]) -> anyhow::Result<Vec<u8>> {
        let key = *self
            .key
            .lock()
            .map_err(|_| anyhow::anyhow!("two-phase key lock poisoned"))?
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("two-phase data key is not unwrapped"))?;
        if envelope.len() < 8 + IV_LEN + TAG_LEN || &envelope[..8] != TWO_PHASE_MAGIC {
            anyhow::bail!("not a FMSH2PH1 two-phase envelope");
        }
        let iv: [u8; IV_LEN] = envelope[8..8 + IV_LEN].try_into()?;
        let tag = &envelope[8 + IV_LEN..8 + IV_LEN + TAG_LEN];
        let ct = &envelope[8 + IV_LEN + TAG_LEN..];
        let mut ct_and_tag = Vec::with_capacity(ct.len() + TAG_LEN);
        ct_and_tag.extend_from_slice(ct);
        ct_and_tag.extend_from_slice(tag);
        aes_gcm_open(&key, &iv, &ct_and_tag)
    }
}
