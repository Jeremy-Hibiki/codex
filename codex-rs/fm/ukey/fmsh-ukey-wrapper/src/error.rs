//! Typed errors over `FM_Result` codes plus SDK last-error text capture.

use crate::ffi::FmResult;
use crate::ffi::{self};

#[derive(Debug, thiserror::Error)]
pub enum UkeyError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("SDK initialization failed (code {code}): {message}")]
    Init { code: FmResult, message: String },

    #[error("device error (code {code}): {message}")]
    Device { code: FmResult, message: String },

    #[error("PIN verification failed (code {code}): {message}")]
    Pin { code: FmResult, message: String },

    #[error("PIN derivation failed (code {code}): {message}")]
    PinDerive { code: FmResult, message: String },

    #[error("container error (code {code}): {message}")]
    Container { code: FmResult, message: String },

    #[error("certificate error (code {code}): {message}")]
    Certificate { code: FmResult, message: String },

    #[error("envelope error (code {code}): {message}")]
    Envelope { code: FmResult, message: String },

    #[error("SDK error (code {code}): {message}")]
    Sdk { code: FmResult, message: String },

    #[error("invalid UTF-8 in SDK string: {0}")]
    Utf8(#[from] std::str::Utf8Error),
}

impl UkeyError {
    /// Maps a non-success `FM_Result` to a typed error, fetching the SDK's
    /// last-error message for context. The SDK guarantees the message never
    /// contains PIN/CEK/plaintext material.
    pub fn from_result(code: FmResult) -> Self {
        let message = last_error_message().unwrap_or_else(|| "unknown SDK error".to_string());
        match code {
            ffi::FM_ERROR_DEVICE_NOT_FOUND | ffi::FM_ERROR_DEVICE_ACCESS => {
                Self::Device { code, message }
            }
            ffi::FM_ERROR_PIN_VERIFY_FAILED => Self::Pin { code, message },
            ffi::FM_ERROR_PIN_DERIVE_FAILED => Self::PinDerive { code, message },
            ffi::FM_ERROR_CONTAINER_NOT_FOUND => Self::Container { code, message },
            ffi::FM_ERROR_CERTIFICATE_INVALID => Self::Certificate { code, message },
            ffi::FM_ERROR_ENVELOPE_INVALID
            | ffi::FM_ERROR_UNSUPPORTED_ALGORITHM
            | ffi::FM_ERROR_RECIPIENT_NOT_FOUND
            | ffi::FM_ERROR_DECRYPT_FAILED
            | ffi::FM_ERROR_PLAINTEXT_INVALID
            | ffi::FM_ERROR_ENCRYPT_FAILED
            | ffi::FM_ERROR_RECIPIENT_INVALID => Self::Envelope { code, message },
            ffi::FM_ERROR_PROVIDER_NOT_FOUND | ffi::FM_ERROR_NOT_INITIALIZED => {
                Self::Init { code, message }
            }
            _ => Self::Sdk { code, message },
        }
    }

    pub fn code(&self) -> Option<FmResult> {
        match self {
            Self::Init { code, .. }
            | Self::Device { code, .. }
            | Self::Pin { code, .. }
            | Self::PinDerive { code, .. }
            | Self::Container { code, .. }
            | Self::Certificate { code, .. }
            | Self::Envelope { code, .. }
            | Self::Sdk { code, .. } => Some(*code),
            Self::Config(_) | Self::Utf8(_) => None,
        }
    }

    /// The original `FM_*` error-code name (e.g. "FM_ERROR_DEVICE_ACCESS"),
    /// exposed to bindings as the error `code`. Non-SDK errors (Config/Utf8)
    /// yield "FM_ERROR_CONFIG" so every error has a non-null code string.
    pub fn code_name(&self) -> &'static str {
        match self.code() {
            Some(c) => fm_result_name(c),
            None => "FM_ERROR_CONFIG",
        }
    }
}

/// Maps an `FM_Result` to its original `FM_*` constant name.
/// Unknown codes yield "FM_ERROR_UNKNOWN".
pub fn fm_result_name(code: FmResult) -> &'static str {
    match code {
        ffi::FM_SUCCESS => "FM_SUCCESS",
        ffi::FM_ERROR_INVALID_ARGUMENT => "FM_ERROR_INVALID_ARGUMENT",
        ffi::FM_ERROR_NOT_SUPPORTED => "FM_ERROR_NOT_SUPPORTED",
        ffi::FM_ERROR_INTERNAL => "FM_ERROR_INTERNAL",
        ffi::FM_ERROR_USE_STREAM_API => "FM_ERROR_USE_STREAM_API",
        ffi::FM_ERROR_BUFFER_TOO_SMALL => "FM_ERROR_BUFFER_TOO_SMALL",
        ffi::FM_ERROR_CALLBACK_FAILED => "FM_ERROR_CALLBACK_FAILED",
        ffi::FM_ERROR_NOT_INITIALIZED => "FM_ERROR_NOT_INITIALIZED",
        ffi::FM_ERROR_ALREADY_INITIALIZED => "FM_ERROR_ALREADY_INITIALIZED",
        ffi::FM_ERROR_PROVIDER_NOT_FOUND => "FM_ERROR_PROVIDER_NOT_FOUND",
        ffi::FM_ERROR_DEVICE_NOT_FOUND => "FM_ERROR_DEVICE_NOT_FOUND",
        ffi::FM_ERROR_DEVICE_ACCESS => "FM_ERROR_DEVICE_ACCESS",
        ffi::FM_ERROR_PIN_VERIFY_FAILED => "FM_ERROR_PIN_VERIFY_FAILED",
        ffi::FM_ERROR_PIN_DERIVE_FAILED => "FM_ERROR_PIN_DERIVE_FAILED",
        ffi::FM_ERROR_CONTAINER_NOT_FOUND => "FM_ERROR_CONTAINER_NOT_FOUND",
        ffi::FM_ERROR_UKEY_OPERATION_FAILED => "FM_ERROR_UKEY_OPERATION_FAILED",
        ffi::FM_ERROR_ENVELOPE_INVALID => "FM_ERROR_ENVELOPE_INVALID",
        ffi::FM_ERROR_UNSUPPORTED_ALGORITHM => "FM_ERROR_UNSUPPORTED_ALGORITHM",
        ffi::FM_ERROR_RECIPIENT_NOT_FOUND => "FM_ERROR_RECIPIENT_NOT_FOUND",
        ffi::FM_ERROR_DECRYPT_FAILED => "FM_ERROR_DECRYPT_FAILED",
        ffi::FM_ERROR_PLAINTEXT_INVALID => "FM_ERROR_PLAINTEXT_INVALID",
        ffi::FM_ERROR_CERTIFICATE_INVALID => "FM_ERROR_CERTIFICATE_INVALID",
        ffi::FM_ERROR_ENCRYPT_FAILED => "FM_ERROR_ENCRYPT_FAILED",
        ffi::FM_ERROR_RECIPIENT_INVALID => "FM_ERROR_RECIPIENT_INVALID",
        _ => "FM_ERROR_UNKNOWN",
    }
}

/// Fetches the thread's last SDK error message via the two-call pattern.
/// The reported size INCLUDES the trailing NUL. Non-ASCII bytes are
/// replaced with '?' (SDK messages occasionally carry provider garbage).
pub fn last_error_message() -> Option<String> {
    unsafe {
        let mut size: u32 = 0;
        let rc = ffi::FM_GetLastErrorMessage(std::ptr::null_mut(), &mut size);
        if rc != ffi::FM_SUCCESS && rc != ffi::FM_ERROR_BUFFER_TOO_SMALL {
            return None;
        }
        if size == 0 {
            return None;
        }
        let mut buf = vec![0u8; size as usize]; // size includes trailing NUL
        let rc = ffi::FM_GetLastErrorMessage(buf.as_mut_ptr() as *mut std::ffi::c_char, &mut size);
        if rc != ffi::FM_SUCCESS {
            return None;
        }
        // Strip trailing NUL(s) and sanitize non-ASCII.
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        let sanitized: String = buf[..end]
            .iter()
            .map(|&b| if b.is_ascii() { b as char } else { '?' })
            .collect();
        Some(sanitized)
    }
}

/// Checks an `FM_Result`; `Ok(())` on success, typed error otherwise.
pub(crate) fn check(code: FmResult) -> Result<(), UkeyError> {
    if code == ffi::FM_SUCCESS {
        Ok(())
    } else {
        Err(UkeyError::from_result(code))
    }
}
