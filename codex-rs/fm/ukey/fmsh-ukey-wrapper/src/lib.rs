//! Safe Rust wrapper around the FMSH UKey SDK (`libfmsh_ukey_sdk.so`).
//!
//! One [`Ukey`] handle owns the process-global SDK lifecycle. All FFI quirks
//! (UTF-32 provider path, two-call buffer pattern, `\0`-vs-`\n` enum output,
//! `ALREADY_INITIALIZED` tolerance) are handled here so consumers — the CLI
//! and the Node.js/Python bindings — never touch the C SDK.

mod error;
pub mod ffi;

pub use error::fm_result_name;
pub use error::last_error_message;
pub use error::UkeyError;

use std::ffi::CString;
use std::path::Path;
use std::sync::OnceLock;

use parking_lot::Mutex;

pub type Result<T, E = UkeyError> = std::result::Result<T, E>;

/// Process-global init success marker, set exactly once on success. A failed
/// initialization is NOT cached, so a later call (e.g. after fixing
/// FMSH_UKEY_PROVIDER) retries instead of returning the stale failure.
static INIT_OK: OnceLock<()> = OnceLock::new();
static INIT_LOCK: Mutex<()> = Mutex::new(());
static FINALIZED: Mutex<bool> = Mutex::new(false);

/// Process-wide handle to the FMSH UKey SDK.
#[derive(Debug)]
pub struct Ukey {
    _private: (),
}

impl Ukey {
    /// Initializes the SDK using the provider path from
    /// `FMSH_UKEY_PROVIDER` (required, no fallback).
    pub fn initialize() -> Result<Self> {
        Self::initialize_with_provider(None::<&Path>)
    }

    /// Initializes the SDK with an explicit provider library path, or
    /// `FMSH_UKEY_PROVIDER` when `None`.
    ///
    /// Idempotent: a second call after a successful init returns a new
    /// handle without re-initializing the SDK.
    pub fn initialize_with_provider<P: AsRef<Path>>(provider: Option<P>) -> Result<Self> {
        let provider = match provider {
            Some(p) => p.as_ref().to_path_buf(),
            None => std::env::var("FMSH_UKEY_PROVIDER")
                .ok()
                .filter(|v| !v.is_empty())
                .map(std::path::PathBuf::from)
                .ok_or_else(|| {
                    UkeyError::Config(
                        "FMSH_UKEY_PROVIDER is not set and no provider path was given; \
                         point it at the GM3000 provider (e.g. linux/provider/libgm3000.1.0.so)"
                            .to_string(),
                    )
                })?,
        };

        if INIT_OK.get().is_none() {
            // Serialize concurrent first-time initialization (parking_lot lock
            // has no poisoning).
            let _guard = INIT_LOCK.lock();
            if INIT_OK.get().is_none() {
                init_sdk(&provider)?;
                let _ = INIT_OK.set(());
            }
        }
        Ok(Self { _private: () })
    }

    /// SDK ABI version: major << 16 | minor.
    pub fn api_version() -> u32 {
        unsafe { ffi::FM_GetApiVersion() }
    }

    /// Names of connected UKey devices.
    ///
    /// SDK 0.3.0+ returns '\n'-separated UTF-8 device names.
    pub fn enum_devices(&self) -> Result<Vec<String>> {
        unsafe {
            let mut size: u32 = 0;
            let rc = ffi::FM_EnumDevices(std::ptr::null_mut(), &mut size);
            if rc != ffi::FM_SUCCESS && rc != ffi::FM_ERROR_BUFFER_TOO_SMALL {
                return Err(UkeyError::from_result(rc));
            }
            if size <= 1 {
                return Ok(Vec::new());
            }
            let mut buf = vec![0u8; size as usize];
            error::check(ffi::FM_EnumDevices(
                buf.as_mut_ptr() as *mut std::ffi::c_char,
                &mut size,
            ))?;
            Ok(buf
                .split(|&b| b == b'\n')
                .filter(|s| !s.is_empty())
                .map(|s| s.strip_suffix(&[b'\0']).unwrap_or(s))
                .map(|s| String::from_utf8_lossy(s).into_owned())
                .collect())
        }
    }

    /// First enumerated device, or an error if none is connected.
    pub fn default_device(&self) -> Result<String> {
        self.enum_devices()?
            .into_iter()
            .next()
            .ok_or_else(|| UkeyError::Config("no UKey device connected".to_string()))
    }

    /// Exports the encryption certificate (DER) from `device`/`container`.
    /// `None` device selects the first enumerated device; `None` container
    /// requires `FMSH_UKEY_CONTAINER` to be set (no built-in default).
    pub fn export_encryption_cert(
        &self,
        device: Option<&str>,
        container: Option<&str>,
    ) -> Result<Vec<u8>> {
        let device = resolve_device(self, device)?;
        let container = resolve_container(container)?;
        let device_c = c_string(&device)?;
        let container_c = c_string(&container)?;

        unsafe {
            let mut len: u64 = 0;
            let rc = ffi::FM_ExportEncryptionCertificateBuffer(
                device_c.as_ptr(),
                container_c.as_ptr(),
                std::ptr::null_mut(),
                &mut len,
            );
            if rc != ffi::FM_SUCCESS && rc != ffi::FM_ERROR_BUFFER_TOO_SMALL {
                return Err(UkeyError::from_result(rc));
            }
            let mut buf = vec![0u8; len as usize];
            error::check(ffi::FM_ExportEncryptionCertificateBuffer(
                device_c.as_ptr(),
                container_c.as_ptr(),
                buf.as_mut_ptr(),
                &mut len,
            ))?;
            buf.truncate(len as usize);
            Ok(buf)
        }
    }

    /// Encrypts `plaintext` into a CMS SM2/SM4 envelope using a DER or PEM
    /// X.509 SM2 encryption certificate. Requires prior initialization even
    /// though no hardware is touched.
    pub fn encrypt(&self, cert: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
        unsafe {
            let mut len: u64 = 0;
            let rc = ffi::FM_EncryptEnvelopeBuffer(
                cert.as_ptr(),
                cert.len() as u64,
                plaintext.as_ptr(),
                plaintext.len() as u64,
                std::ptr::null_mut(),
                &mut len,
            );
            if rc != ffi::FM_SUCCESS && rc != ffi::FM_ERROR_BUFFER_TOO_SMALL {
                return Err(UkeyError::from_result(rc));
            }
            let mut buf = vec![0u8; len as usize];
            error::check(ffi::FM_EncryptEnvelopeBuffer(
                cert.as_ptr(),
                cert.len() as u64,
                plaintext.as_ptr(),
                plaintext.len() as u64,
                buf.as_mut_ptr(),
                &mut len,
            ))?;
            buf.truncate(len as usize);
            Ok(buf)
        }
    }

    /// Decrypts a CMS envelope through the UKey container private key.
    pub fn decrypt(
        &self,
        envelope: &[u8],
        device: Option<&str>,
        container: Option<&str>,
    ) -> Result<Vec<u8>> {
        let device = resolve_device(self, device)?;
        let container = resolve_container(container)?;
        let device_c = c_string(&device)?;
        let container_c = c_string(&container)?;

        unsafe {
            let mut len: u64 = 0;
            let rc = ffi::FM_DecryptEnvelopeBuffer(
                device_c.as_ptr(),
                container_c.as_ptr(),
                envelope.as_ptr(),
                envelope.len() as u64,
                std::ptr::null_mut(),
                &mut len,
            );
            if rc != ffi::FM_SUCCESS && rc != ffi::FM_ERROR_BUFFER_TOO_SMALL {
                return Err(UkeyError::from_result(rc));
            }
            let mut buf = vec![0u8; len as usize];
            error::check(ffi::FM_DecryptEnvelopeBuffer(
                device_c.as_ptr(),
                container_c.as_ptr(),
                envelope.as_ptr(),
                envelope.len() as u64,
                buf.as_mut_ptr(),
                &mut len,
            ))?;
            buf.truncate(len as usize);
            Ok(buf)
        }
    }

    /// Encrypts every file in `dir` into CMS envelopes sealed to `cert`.
    /// Writes `<filename>.enc` next to each source file (input files are
    /// never modified). Returns the list of created output paths.
    pub fn batch_encrypt_directory(
        &self,
        cert: &[u8],
        dir: &Path,
    ) -> Result<Vec<std::path::PathBuf>> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(dir)
            .map_err(|e| UkeyError::Config(format!("reading dir {}: {e}", dir.display())))?
        {
            let entry = entry.map_err(|e| UkeyError::Config(format!("reading dir entry: {e}")))?;
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            // Skip already-encrypted files to keep the operation idempotent.
            if name.ends_with(".enc") {
                continue;
            }
            let plaintext = std::fs::read(&path)
                .map_err(|e| UkeyError::Config(format!("reading {}: {e}", path.display())))?;
            let envelope = self.encrypt(cert, &plaintext)?;
            let out_path = path.with_extension("enc");
            std::fs::write(&out_path, &envelope)
                .map_err(|e| UkeyError::Config(format!("writing {}: {e}", out_path.display())))?;
            out.push(out_path);
        }
        Ok(out)
    }

    /// Explicitly finalizes the SDK. Safe to call more than once. Prefer
    /// calling this before process exit: skipping it can cause exit 139
    /// from SDK/provider destructor ordering.
    pub fn finalize() {
        let mut done = FINALIZED.lock();
        if !*done {
            unsafe { ffi::FM_Finalize() };
            *done = true;
        }
    }
}

impl Drop for Ukey {
    fn drop(&mut self) {
        Self::finalize();
    }
}

/// Process-lifetime shared handle whose `Drop` is suppressed — for FFI
/// facades and bindings where consumers expect init-once/use-many semantics
/// and the SDK outlives any single call. End the SDK explicitly with
/// [`Ukey::finalize`]. `Ukey` is a ZST: no allocation is leaked.
pub fn shared() -> Result<&'static Ukey> {
    shared_with_provider(None)
}

/// Like [`shared`], but the first (not-yet-initialized) call may supply an
/// explicit provider path instead of relying on `FMSH_UKEY_PROVIDER`. The
/// provider is only used on the initializing call; once the shared handle
/// exists, later arguments are ignored (single process-global SDK instance).
pub fn shared_with_provider(provider: Option<&str>) -> Result<&'static Ukey> {
    use std::sync::OnceLock;
    // Only a successfully-initialized handle is cached. A failed initialization
    // is NOT cached, so a later call (e.g. after fixing the provider path)
    // retries instead of returning the stale failure forever.
    static SHARED: OnceLock<std::mem::ManuallyDrop<Ukey>> = OnceLock::new();
    static SHARED_LOCK: Mutex<()> = Mutex::new(());

    if let Some(h) = SHARED.get() {
        return Ok(h);
    }
    // Serialize concurrent first-time initialization (parking_lot: no poison).
    let _guard = SHARED_LOCK.lock();
    if let Some(h) = SHARED.get() {
        return Ok(h);
    }
    let ukey = Ukey::initialize_with_provider(provider).map(std::mem::ManuallyDrop::new)?;
    // Another thread cannot have raced us here (we hold SHARED_LOCK and SHARED
    // was empty), but set() is total: ignore the impossible Err.
    let _ = SHARED.set(ukey);
    Ok(SHARED.get().expect("SHARED just initialized"))
}

// Helpers

fn init_sdk(provider: &Path) -> Result<()> {
    // wchar_t on Linux is 4-byte UTF-32: encode code points + NUL.
    let provider_str = provider.to_string_lossy();
    let provider_wide: Vec<u32> = provider_str
        .chars()
        .map(|c| c as u32)
        .chain(std::iter::once(0))
        .collect();

    let opts = ffi::FmInitOptions {
        struct_size: std::mem::size_of::<ffi::FmInitOptions>() as u32,
        provider_library_path: provider_wide.as_ptr(),
        flags: 0,
        reserved: 0,
    };
    let rc = unsafe { ffi::FM_Initialize(&opts) };
    if rc == ffi::FM_SUCCESS || rc == ffi::FM_ERROR_ALREADY_INITIALIZED {
        Ok(())
    } else {
        Err(UkeyError::Init {
            code: rc,
            message: last_error_message().unwrap_or_else(|| "initialization failed".into()),
        })
    }
}

fn resolve_device(ukey: &Ukey, device: Option<&str>) -> Result<String> {
    match device {
        Some(d) => Ok(d.to_string()),
        None => ukey.default_device(),
    }
}

fn resolve_container(container: Option<&str>) -> Result<String> {
    container
        .map(str::to_string)
        .or_else(|| std::env::var("FMSH_UKEY_CONTAINER").ok())
        .filter(|c| !c.is_empty())
        .ok_or_else(|| {
            UkeyError::Config("no container given and FMSH_UKEY_CONTAINER is not set".to_string())
        })
}

fn c_string(s: &str) -> Result<CString> {
    CString::new(s).map_err(|_| UkeyError::Config(format!("string contains NUL: {s:?}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_without_provider_is_config_error() {
        std::env::remove_var("FMSH_UKEY_PROVIDER");
        match Ukey::initialize() {
            Err(UkeyError::Config(msg)) => assert!(msg.contains("FMSH_UKEY_PROVIDER")),
            // A machine with the env var legitimately set just passes init.
            other => {
                if std::env::var("FMSH_UKEY_PROVIDER").is_err() {
                    panic!("expected Config error, got {other:?}");
                }
            }
        }
    }

    #[test]
    fn utf32_encoding_handles_unicode() {
        let wide: Vec<u32> = "/tmp/密钥.so"
            .chars()
            .map(|c| c as u32)
            .chain(std::iter::once(0))
            .collect();
        assert_eq!(wide.last(), Some(&0));
        assert_eq!(wide.len(), "/tmp/密钥.so".chars().count() + 1);
    }
}
