//! Digital-envelope SDK boundary.

use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

/// Errors surfaced by the encrypted package pipeline.
#[derive(Debug, thiserror::Error)]
pub enum EnvelopeError {
    #[error("encrypted package not found: {0}")]
    PackageNotFound(String),
    #[error("hardware key required for decryption")]
    HardwareKeyRequired,
    #[error("decryption failed: {0}")]
    Decrypt(String),
    #[error("no envelope SDK configured for encrypted skills")]
    SdkUnavailable,
    #[error("internal encrypted-skill error: {0}")]
    Internal(String),
    #[error("invalid package entry `{path}`: {reason}")]
    InvalidEntry { path: String, reason: String },
    #[error("encrypted package has too many entries (max {max})")]
    TooManyEntries { max: usize },
    #[error("decrypted package exceeds the {max_bytes} byte size limit")]
    PackageTooLarge { max_bytes: u64 },
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Hard cap on the number of entries a decrypted package may contain.
pub const MAX_PACKAGE_ENTRIES: usize = 512;
/// Hard cap on the total decompressed size of a decrypted package.
pub const MAX_PACKAGE_BYTES: u64 = 16 * 1024 * 1024;

/// One file inside a decrypted skill package, addressed relative to the
/// package root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageEntry {
    pub rel_path: PathBuf,
    pub contents: Vec<u8>,
}

/// Decrypts `<name>.zip.enc` packages into in-memory entries.
///
/// Implementations own the crypto and key management; this crate owns the
/// package layout contract, safety validation, and directory lifecycle.
pub trait EnvelopeSdk: Send + Sync {
    fn decrypt_package(&self, package_path: &Path) -> Result<Vec<PackageEntry>, EnvelopeError>;
}

/// Fail-closed SDK used when no real digital-envelope SDK is configured.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnavailableSdk;

impl EnvelopeSdk for UnavailableSdk {
    fn decrypt_package(&self, _package_path: &Path) -> Result<Vec<PackageEntry>, EnvelopeError> {
        Err(EnvelopeError::SdkUnavailable)
    }
}

/// Selects which envelope SDK implementation the host should construct.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum SdkKind {
    #[default]
    Unavailable,
    /// Test-only SDK that decrypts plain ZIP packages (`.zip.enc` is a zip).
    TestZip,
    /// Identity transform: the `.enc` package is the plain ZIP renamed.
    Noop,
    /// Software digital envelope (HPKE default or standard CMS SM2-SM4-CBC).
    /// Backed by `fmsh-ukey-cipher` (compiled on Linux x86_64 gnu).
    Software {
        algorithm: SdkSoftwareAlgorithm,
        privkey: Option<PathBuf>,
    },
    /// CMS SM2/SM4 envelope through the FMSH UKey SDK (compiled on Linux
    /// x86_64 gnu).
    UKey,
    /// UKey two-phase: one UKey call unwraps a per-skill `key.enc`, then
    /// every package is decrypted in software AES-256-GCM with the in-memory
    /// key (compiled on Linux x86_64 gnu).
    UKeyTwoPhase { key_envelope: String },
}

/// Selectable algorithm for the software envelope backend.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SdkSoftwareAlgorithm {
    /// HPKE (RFC 9180) base mode: X25519 + HKDF-SHA256 + AES-256-GCM.
    #[default]
    HpkeX25519Aes256Gcm,
    /// Standard CMS EnvelopedData (SM2 key transport + SM4-CBC, GM/T 0010).
    Sm2Sm4Cbc,
}

pub fn sdk_for(kind: SdkKind) -> Arc<dyn EnvelopeSdk> {
    match kind {
        SdkKind::Unavailable => Arc::new(UnavailableSdk),
        SdkKind::TestZip => Arc::new(TestZipSdk),
        SdkKind::Noop => Arc::new(NoopEnvelopeSdk),
        #[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
        SdkKind::Software { algorithm, privkey } => {
            match fmsh::SoftwareSdk::new(algorithm, privkey.as_deref()) {
                Ok(sdk) => Arc::new(sdk),
                Err(error) => {
                    tracing::warn!(error = %error, "software envelope SDK unavailable; falling back to fail-closed");
                    Arc::new(UnavailableSdk)
                }
            }
        }
        #[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
        SdkKind::UKey => match fmsh::UKeySdk::new() {
            Ok(sdk) => Arc::new(sdk),
            Err(error) => {
                tracing::warn!(error = %error, "fmsh-ukey SDK unavailable; falling back to fail-closed");
                Arc::new(UnavailableSdk)
            }
        },
        #[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
        SdkKind::UKeyTwoPhase { key_envelope } => match fmsh::UkeyTwoPhaseSdk::new(key_envelope) {
            Ok(sdk) => Arc::new(sdk),
            Err(error) => {
                tracing::warn!(error = %error, "ukey-two-phase SDK unavailable; falling back to fail-closed");
                Arc::new(UnavailableSdk)
            }
        },
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu")))]
        SdkKind::Software { .. } | SdkKind::UKey | SdkKind::UKeyTwoPhase { .. } => {
            tracing::warn!("encrypted-skill SDK kind is not compiled in on this platform");
            Arc::new(UnavailableSdk)
        }
    }
}

/// Decrypts plain ZIP packages for integration tests. Mirrors the opencode
/// reference implementation's simulated encryption (`.zip.enc` = renamed zip).
#[derive(Debug, Clone, Copy, Default)]
pub struct TestZipSdk;

impl EnvelopeSdk for TestZipSdk {
    fn decrypt_package(&self, package_path: &Path) -> Result<Vec<PackageEntry>, EnvelopeError> {
        let file = std::fs::File::open(package_path)
            .map_err(|_| EnvelopeError::PackageNotFound(package_path.display().to_string()))?;
        let mut bytes = Vec::new();
        Read::read_to_end(&mut file.take(MAX_PACKAGE_BYTES + 1), &mut bytes)
            .map_err(EnvelopeError::Io)?;
        if bytes.len() as u64 > MAX_PACKAGE_BYTES {
            return Err(EnvelopeError::PackageTooLarge {
                max_bytes: MAX_PACKAGE_BYTES,
            });
        }
        parse_zip_bytes(&bytes)
    }
}

/// Identity transform: the `.enc` package is the plain ZIP renamed.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopEnvelopeSdk;

impl EnvelopeSdk for NoopEnvelopeSdk {
    fn decrypt_package(&self, package_path: &Path) -> Result<Vec<PackageEntry>, EnvelopeError> {
        let file = std::fs::File::open(package_path)
            .map_err(|_| EnvelopeError::PackageNotFound(package_path.display().to_string()))?;
        let mut bytes = Vec::new();
        Read::read_to_end(&mut file.take(MAX_PACKAGE_BYTES + 1), &mut bytes)
            .map_err(EnvelopeError::Io)?;
        if bytes.len() as u64 > MAX_PACKAGE_BYTES {
            return Err(EnvelopeError::PackageTooLarge {
                max_bytes: MAX_PACKAGE_BYTES,
            });
        }
        parse_zip_bytes(&bytes)
    }
}

/// Parses a decrypted package zip into bounded entries, applying the same
/// entry-count and decompressed-size caps regardless of the envelope backend.
fn parse_zip_bytes(zip_bytes: &[u8]) -> Result<Vec<PackageEntry>, EnvelopeError> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(zip_bytes))
        .map_err(|err| EnvelopeError::Decrypt(format!("invalid zip package: {err}")))?;
    if archive.len() > MAX_PACKAGE_ENTRIES {
        return Err(EnvelopeError::TooManyEntries {
            max: MAX_PACKAGE_ENTRIES,
        });
    }
    let mut entries = Vec::new();
    let mut total_bytes = 0u64;
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|err| EnvelopeError::Decrypt(format!("zip entry {index}: {err}")))?;
        if entry.is_dir() {
            continue;
        }
        let rel_path = PathBuf::from(entry.name());
        let remaining = MAX_PACKAGE_BYTES.saturating_sub(total_bytes);
        let mut contents = Vec::new();
        let mut limited = entry.take(remaining + 1);
        Read::read_to_end(&mut limited, &mut contents)
            .map_err(|err| EnvelopeError::Decrypt(format!("zip read: {err}")))?;
        if contents.len() as u64 > remaining {
            return Err(EnvelopeError::PackageTooLarge {
                max_bytes: MAX_PACKAGE_BYTES,
            });
        }
        total_bytes += contents.len() as u64;
        entries.push(PackageEntry { rel_path, contents });
    }
    Ok(entries)
}

#[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
mod fmsh {
    use std::collections::HashMap;
    use std::path::Path;
    use std::sync::Arc;
    use std::sync::RwLock;

    use fmsh_ukey_core::Cipher;
    use fmsh_ukey_core::SoftwareAlgorithm;
    use fmsh_ukey_core::SoftwareCipher;
    use fmsh_ukey_core::UkeyCipher;
    use fmsh_ukey_core::UkeyKeyWrap;
    use fmsh_ukey_core::UkeyTwoPhaseCipher;

    use super::EnvelopeError;
    use super::EnvelopeSdk;
    use super::PackageEntry;
    use super::SdkSoftwareAlgorithm;
    use super::parse_zip_bytes;

    fn software_algorithm(algorithm: SdkSoftwareAlgorithm) -> SoftwareAlgorithm {
        match algorithm {
            SdkSoftwareAlgorithm::HpkeX25519Aes256Gcm => SoftwareAlgorithm::HpkeX25519Aes256Gcm,
            SdkSoftwareAlgorithm::Sm2Sm4Cbc => SoftwareAlgorithm::Sm2Sm4Cbc,
        }
    }

    /// Software envelope backend. The private key PEM is copied into an
    /// anonymous memfd so key material never touches disk.
    pub(crate) struct SoftwareSdk {
        cipher: SoftwareCipher,
    }

    impl SoftwareSdk {
        pub(crate) fn new(
            algorithm: SdkSoftwareAlgorithm,
            privkey: Option<&Path>,
        ) -> Result<Self, EnvelopeError> {
            let Some(privkey) = privkey else {
                return Err(EnvelopeError::Decrypt(
                    "software mode requires a private key (encrypted_skills.software_privkey)"
                        .to_string(),
                ));
            };
            let pem = std::fs::read(privkey).map_err(EnvelopeError::Io)?;
            let key_file =
                crate::memfd::write_key_memfd(&pem, "fmsh-software-key").map_err(|err| {
                    EnvelopeError::Decrypt(format!("creating in-memory key file: {err:#}"))
                })?;
            let cipher = SoftwareCipher::from_priv_file(
                &crate::memfd::fd_path(&key_file),
                software_algorithm(algorithm),
            )
            .map_err(|err| {
                EnvelopeError::Decrypt(format!("loading software private key: {err:#}"))
            })?;
            Ok(Self { cipher })
        }
    }

    impl EnvelopeSdk for SoftwareSdk {
        fn decrypt_package(&self, package_path: &Path) -> Result<Vec<PackageEntry>, EnvelopeError> {
            let envelope = std::fs::read(package_path).map_err(EnvelopeError::Io)?;
            let zip_bytes = self.cipher.decrypt(&envelope).map_err(|err| {
                EnvelopeError::Decrypt(format!("software decrypt failed: {err:#}"))
            })?;
            parse_zip_bytes(&zip_bytes)
        }
    }

    /// Decrypts `fmsh-ukey-enc` skill packages (`.zip.enc` = CMS SM2/SM4
    /// envelope of the skill zip) through the FMSH UKey SDK.
    pub(crate) struct UKeySdk {
        cipher: Arc<UkeyCipher>,
    }

    impl UKeySdk {
        pub(crate) fn new() -> Result<Self, EnvelopeError> {
            let cipher = UkeyCipher::new(None, None, None)
                .map_err(|_| EnvelopeError::HardwareKeyRequired)?;
            Ok(Self {
                cipher: Arc::new(cipher),
            })
        }
    }

    impl EnvelopeSdk for UKeySdk {
        fn decrypt_package(&self, package_path: &Path) -> Result<Vec<PackageEntry>, EnvelopeError> {
            let envelope = std::fs::read(package_path).map_err(EnvelopeError::Io)?;
            let zip_bytes = self
                .cipher
                .decrypt(&envelope)
                .map_err(|err| EnvelopeError::Decrypt(format!("ukey decrypt failed: {err:#}")))?;
            parse_zip_bytes(&zip_bytes)
        }
    }

    /// Two-phase backend: each skill carries a `key.enc` (a CMS-wrapped
    /// 32-byte AES key). One UKey call unwraps it; the key stays in process
    /// memory and every package of that skill decrypts in software AES-GCM.
    /// The cache is content-addressed: skills that share the same `key.enc`
    /// bytes unwrap the envelope exactly once and reuse the same in-memory key.
    pub(crate) struct UkeyTwoPhaseSdk {
        key_wrap: Arc<dyn fmsh_ukey_core::KeyWrap>,
        key_envelope: String,
        ciphers: RwLock<HashMap<Vec<u8>, Arc<UkeyTwoPhaseCipher>>>,
    }

    impl UkeyTwoPhaseSdk {
        pub(crate) fn new(key_envelope: String) -> Result<Self, EnvelopeError> {
            let key_wrap = Arc::new(
                UkeyKeyWrap::new(None, None, None)
                    .map_err(|_| EnvelopeError::HardwareKeyRequired)?,
            );
            Ok(Self::with_key_wrap(key_envelope, key_wrap))
        }

        pub(crate) fn with_key_wrap(
            key_envelope: String,
            key_wrap: Arc<dyn fmsh_ukey_core::KeyWrap>,
        ) -> Self {
            Self {
                key_wrap,
                key_envelope,
                ciphers: RwLock::new(HashMap::new()),
            }
        }

        fn cipher_for(
            &self,
            package_path: &Path,
        ) -> Result<Arc<UkeyTwoPhaseCipher>, EnvelopeError> {
            let key_path = package_path.with_file_name(&self.key_envelope);
            let wrapped = std::fs::read(&key_path)
                .map_err(|_| EnvelopeError::PackageNotFound(key_path.display().to_string()))?;
            // Content-addressed: two skills pointing at byte-identical key
            // envelopes (shared key material) unwrap exactly once. The wrapped
            // bytes are the cache key, so no extra digest dependency is needed.
            if let Some(cipher) = self
                .ciphers
                .read()
                .ok()
                .and_then(|guard| guard.get(&wrapped).cloned())
            {
                return Ok(cipher);
            }
            let cipher = Arc::new(UkeyTwoPhaseCipher::new(Arc::clone(&self.key_wrap)));
            cipher.unwrap_key(&wrapped).map_err(|err| {
                EnvelopeError::Decrypt(format!("unwrapping key envelope: {err:#}"))
            })?;
            self.ciphers
                .write()
                .map_err(|_| EnvelopeError::Internal("two-phase cache poisoned".into()))?
                .insert(wrapped, Arc::clone(&cipher));
            Ok(cipher)
        }
    }

    impl EnvelopeSdk for UkeyTwoPhaseSdk {
        fn decrypt_package(&self, package_path: &Path) -> Result<Vec<PackageEntry>, EnvelopeError> {
            let cipher = self.cipher_for(package_path)?;
            let envelope = std::fs::read(package_path).map_err(EnvelopeError::Io)?;
            let zip_bytes = cipher.decrypt(&envelope).map_err(|err| {
                EnvelopeError::Decrypt(format!("two-phase decrypt failed: {err:#}"))
            })?;
            parse_zip_bytes(&zip_bytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn test_zip_sdk_extracts_package_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let package = tmp.path().join("secret.zip.enc");
        let file = std::fs::File::create(&package).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("SKILL.md", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"# Encrypted skill").unwrap();
        writer
            .start_file("scripts/build.sh", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"#!/bin/sh").unwrap();
        writer.finish().unwrap();

        let entries = TestZipSdk.decrypt_package(&package).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].rel_path, PathBuf::from("SKILL.md"));
        assert_eq!(entries[0].contents, b"# Encrypted skill");
        assert_eq!(entries[1].rel_path, PathBuf::from("scripts/build.sh"));
    }

    #[test]
    fn test_zip_sdk_missing_package_is_not_found() {
        assert!(matches!(
            TestZipSdk.decrypt_package(Path::new("/nonexistent/secret.zip.enc")),
            Err(EnvelopeError::PackageNotFound(_))
        ));
    }

    #[test]
    fn test_zip_sdk_rejects_oversized_packages() {
        let tmp = tempfile::tempdir().unwrap();
        let package = tmp.path().join("big.zip.enc");
        let file = std::fs::File::create(&package).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file(
                "big.bin",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        writer
            .write_all(&vec![0u8; (MAX_PACKAGE_BYTES + 1) as usize])
            .unwrap();
        writer.finish().unwrap();

        assert!(matches!(
            TestZipSdk.decrypt_package(&package),
            Err(EnvelopeError::PackageTooLarge { .. })
        ));
    }

    #[test]
    fn test_zip_sdk_rejects_too_many_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let package = tmp.path().join("many.zip.enc");
        let file = std::fs::File::create(&package).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        for index in 0..MAX_PACKAGE_ENTRIES + 1 {
            writer
                .start_file(
                    format!("f{index}.txt"),
                    zip::write::SimpleFileOptions::default(),
                )
                .unwrap();
            writer.write_all(b"x").unwrap();
        }
        writer.finish().unwrap();

        assert!(matches!(
            TestZipSdk.decrypt_package(&package),
            Err(EnvelopeError::TooManyEntries { .. })
        ));
    }

    #[test]
    fn sdk_for_selects_implementations() {
        assert!(matches!(
            sdk_for(SdkKind::Unavailable).decrypt_package(Path::new("/x.zip.enc")),
            Err(EnvelopeError::SdkUnavailable)
        ));
        assert!(matches!(sdk_for(SdkKind::TestZip).as_ref(), _));
        assert!(matches!(sdk_for(SdkKind::Noop).as_ref(), _));
        #[cfg(not(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu")))]
        {
            // The envelope backends are only compiled on the platform where
            // the vendored SDK ships; elsewhere they fail closed.
            assert!(matches!(
                sdk_for(SdkKind::UKey).decrypt_package(Path::new("/x.zip.enc")),
                Err(EnvelopeError::SdkUnavailable)
            ));
            assert!(matches!(
                sdk_for(SdkKind::Software {
                    algorithm: SdkSoftwareAlgorithm::default(),
                    privkey: None,
                })
                .decrypt_package(Path::new("/x.zip.enc")),
                Err(EnvelopeError::SdkUnavailable)
            ));
            assert!(matches!(
                sdk_for(SdkKind::UKeyTwoPhase {
                    key_envelope: "key.enc".to_string(),
                })
                .decrypt_package(Path::new("/x.zip.enc")),
                Err(EnvelopeError::SdkUnavailable)
            ));
        }
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
    #[test]
    fn software_sdk_decrypts_hpke_package() {
        use fmsh_ukey_core::Cipher;
        use fmsh_ukey_core::SoftwareAlgorithm;
        use fmsh_ukey_core::SoftwareCipher;

        let tmp = tempfile::tempdir().unwrap();
        let pub_path = tmp.path().join("enc.pub.pem");
        let priv_path = tmp.path().join("enc.priv.pem");
        let cipher = SoftwareCipher::generate_to_files(
            &pub_path,
            &priv_path,
            SoftwareAlgorithm::HpkeX25519Aes256Gcm,
        )
        .unwrap();

        let mut zip_buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buf));
            writer
                .start_file("SKILL.md", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"# secret").unwrap();
            writer.finish().unwrap();
        }

        let envelope = cipher.encrypt(&zip_buf).unwrap();
        let package = tmp.path().join("secret.zip.enc");
        std::fs::write(&package, &envelope).unwrap();

        let sdk = super::fmsh::SoftwareSdk::new(
            SdkSoftwareAlgorithm::HpkeX25519Aes256Gcm,
            Some(&priv_path),
        )
        .unwrap();
        let entries = sdk.decrypt_package(&package).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].rel_path, PathBuf::from("SKILL.md"));
        assert_eq!(entries[0].contents, b"# secret");
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
    #[test]
    fn software_sdk_decrypts_sm2_cms_package() {
        use fmsh_ukey_core::Cipher;
        use fmsh_ukey_core::SoftwareAlgorithm;
        use fmsh_ukey_core::SoftwareCipher;

        let tmp = tempfile::tempdir().unwrap();
        let pub_path = tmp.path().join("enc.pub.pem");
        let priv_path = tmp.path().join("enc.priv.pem");
        let cipher =
            SoftwareCipher::generate_to_files(&pub_path, &priv_path, SoftwareAlgorithm::Sm2Sm4Cbc)
                .unwrap();

        let mut zip_buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buf));
            writer
                .start_file("SKILL.md", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"# secret").unwrap();
            writer.finish().unwrap();
        }

        let envelope = cipher.encrypt(&zip_buf).unwrap();
        let package = tmp.path().join("secret.zip.enc");
        std::fs::write(&package, &envelope).unwrap();

        let sdk = super::fmsh::SoftwareSdk::new(SdkSoftwareAlgorithm::Sm2Sm4Cbc, Some(&priv_path))
            .unwrap();
        let entries = sdk.decrypt_package(&package).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].rel_path, PathBuf::from("SKILL.md"));
        assert_eq!(entries[0].contents, b"# secret");
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
    #[test]
    fn two_phase_sdk_decrypts_package_with_in_memory_key() {
        use std::sync::Arc;

        use fmsh_ukey_core::Cipher;
        use fmsh_ukey_core::KeyWrap;
        use fmsh_ukey_core::SoftwareAlgorithm;
        use fmsh_ukey_core::SoftwareCipher;
        use fmsh_ukey_core::UkeyTwoPhaseCipher;

        struct SoftwareKeyWrap(SoftwareCipher);

        impl KeyWrap for SoftwareKeyWrap {
            fn wrap(&self, plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
                self.0.encrypt(plaintext)
            }

            fn unwrap(&self, envelope: &[u8]) -> anyhow::Result<Vec<u8>> {
                self.0.decrypt(envelope)
            }
        }

        let tmp = tempfile::tempdir().unwrap();
        let pub_path = tmp.path().join("key.pub.pem");
        let priv_path = tmp.path().join("key.priv.pem");
        let software = SoftwareCipher::generate_to_files(
            &pub_path,
            &priv_path,
            SoftwareAlgorithm::HpkeX25519Aes256Gcm,
        )
        .unwrap();
        let wrap = Arc::new(SoftwareKeyWrap(software));

        let two_phase = UkeyTwoPhaseCipher::new(wrap.clone());
        let wrapped_key = two_phase.wrap_key().unwrap();
        std::fs::write(tmp.path().join("key.enc"), &wrapped_key).unwrap();

        let mut zip_buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buf));
            writer
                .start_file("SKILL.md", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"# secret").unwrap();
            writer.finish().unwrap();
        }
        let encrypted = two_phase.encrypt(&zip_buf).unwrap();
        let package = tmp.path().join("secret.zip.enc");
        std::fs::write(&package, &encrypted).unwrap();

        let sdk = super::fmsh::UkeyTwoPhaseSdk::with_key_wrap("key.enc".to_string(), wrap);
        let entries = sdk.decrypt_package(&package).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].rel_path, PathBuf::from("SKILL.md"));
        assert_eq!(entries[0].contents, b"# secret");
    }

    #[cfg(all(target_os = "linux", target_arch = "x86_64", target_env = "gnu"))]
    #[test]
    fn shared_key_envelope_is_unwrapped_exactly_once_across_skills() {
        use std::sync::Arc;
        use std::sync::atomic::AtomicUsize;
        use std::sync::atomic::Ordering;

        use fmsh_ukey_core::Cipher;
        use fmsh_ukey_core::KeyWrap;
        use fmsh_ukey_core::SoftwareAlgorithm;
        use fmsh_ukey_core::SoftwareCipher;
        use fmsh_ukey_core::UkeyTwoPhaseCipher;

        struct SoftwareKeyWrap(Arc<SoftwareCipher>);

        impl KeyWrap for SoftwareKeyWrap {
            fn wrap(&self, plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
                self.0.encrypt(plaintext)
            }

            fn unwrap(&self, envelope: &[u8]) -> anyhow::Result<Vec<u8>> {
                self.0.decrypt(envelope)
            }
        }

        struct CountingKeyWrap(SoftwareKeyWrap, Arc<AtomicUsize>);

        impl KeyWrap for CountingKeyWrap {
            fn wrap(&self, plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
                self.0.wrap(plaintext)
            }

            fn unwrap(&self, envelope: &[u8]) -> anyhow::Result<Vec<u8>> {
                self.1.fetch_add(1, Ordering::SeqCst);
                self.0.unwrap(envelope)
            }
        }

        let tmp = tempfile::tempdir().unwrap();
        let pub_path = tmp.path().join("key.pub.pem");
        let priv_path = tmp.path().join("key.priv.pem");
        let software = Arc::new(
            SoftwareCipher::generate_to_files(
                &pub_path,
                &priv_path,
                SoftwareAlgorithm::HpkeX25519Aes256Gcm,
            )
            .unwrap(),
        );

        // Encryption side and the SDK's decryption side share one SoftwareCipher
        // (the same key pair), so the wrapped key and packages decrypt back.
        let encrypt_wrap = Arc::new(SoftwareKeyWrap(Arc::clone(&software)));
        let two_phase = UkeyTwoPhaseCipher::new(encrypt_wrap);
        let wrapped_key = two_phase.wrap_key().unwrap();
        let unwrap_calls = Arc::new(AtomicUsize::new(0));
        let sdk = super::fmsh::UkeyTwoPhaseSdk::with_key_wrap(
            "key.enc".to_string(),
            Arc::new(CountingKeyWrap(
                SoftwareKeyWrap(software),
                unwrap_calls.clone(),
            )),
        );

        // Two skills in different directories share the SAME key.enc bytes.
        let mut zip_buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut zip_buf));
            writer
                .start_file("SKILL.md", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(b"# shared secret").unwrap();
            writer.finish().unwrap();
        }
        let encrypted = two_phase.encrypt(&zip_buf).unwrap();

        let skill_a = tmp.path().join("skill-a");
        let skill_b = tmp.path().join("skill-b");
        std::fs::create_dir_all(&skill_a).unwrap();
        std::fs::create_dir_all(&skill_b).unwrap();
        std::fs::write(skill_a.join("key.enc"), &wrapped_key).unwrap();
        std::fs::write(skill_b.join("key.enc"), &wrapped_key).unwrap();
        std::fs::write(skill_a.join("a.zip.enc"), &encrypted).unwrap();
        std::fs::write(skill_b.join("b.zip.enc"), &encrypted).unwrap();

        let a = sdk.decrypt_package(&skill_a.join("a.zip.enc")).unwrap();
        let b = sdk.decrypt_package(&skill_b.join("b.zip.enc")).unwrap();
        assert_eq!(a[0].contents, b"# shared secret");
        assert_eq!(b[0].contents, b"# shared secret");
        assert_eq!(
            unwrap_calls.load(Ordering::SeqCst),
            1,
            "shared key envelope must be unwrapped exactly once, not once per skill"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn memfd_roundtrips_key_bytes_without_disk_entry() {
        let bytes = b"fmsh-software-key-material".to_vec();
        let file = crate::memfd::write_key_memfd(&bytes, "fmsh-test").unwrap();
        let path = crate::memfd::fd_path(&file);
        let read_back = std::fs::read(&path).unwrap();
        assert_eq!(read_back, bytes);
    }
}
