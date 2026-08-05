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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SdkKind {
    #[default]
    Unavailable,
    /// Test-only SDK that decrypts plain ZIP packages (`.zip.enc` is a zip).
    TestZip,
}

pub fn sdk_for(kind: SdkKind) -> Arc<dyn EnvelopeSdk> {
    match kind {
        SdkKind::Unavailable => Arc::new(UnavailableSdk),
        SdkKind::TestZip => Arc::new(TestZipSdk),
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
        let mut archive = zip::ZipArchive::new(file)
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
    }
}
