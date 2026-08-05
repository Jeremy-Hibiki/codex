//! Hand-written FFI declarations for the FMSH UKey SDK C ABI (`fm_api.h`).
//!
//! The SDK is a flat C API of 7 functions — small enough that bindgen is not
//! worth a libclang build dependency. Quirks encoded here:
//! - `FM_INIT_OPTIONS.provider_library_path` is `const wchar_t*`, i.e. 4-byte
//!   UTF-32 on Linux → declared as `*const u32`.
//! - All buffer APIs use the two-call pattern (null out-buffer → required
//!   size, then allocate and call again).

pub type FmResult = u32;

pub const FM_SUCCESS: FmResult = 0;
pub const FM_ERROR_INVALID_ARGUMENT: FmResult = 1;
pub const FM_ERROR_NOT_SUPPORTED: FmResult = 2;
pub const FM_ERROR_INTERNAL: FmResult = 3;
pub const FM_ERROR_USE_STREAM_API: FmResult = 4;
pub const FM_ERROR_BUFFER_TOO_SMALL: FmResult = 5;
pub const FM_ERROR_CALLBACK_FAILED: FmResult = 6;
pub const FM_ERROR_NOT_INITIALIZED: FmResult = 7;
pub const FM_ERROR_ALREADY_INITIALIZED: FmResult = 8;
pub const FM_ERROR_PROVIDER_NOT_FOUND: FmResult = 9;

pub const FM_ERROR_DEVICE_NOT_FOUND: FmResult = 0x0001_0001;
pub const FM_ERROR_DEVICE_ACCESS: FmResult = 0x0001_0002;
pub const FM_ERROR_PIN_VERIFY_FAILED: FmResult = 0x0001_0003;
pub const FM_ERROR_CONTAINER_NOT_FOUND: FmResult = 0x0001_0004;
pub const FM_ERROR_UKEY_OPERATION_FAILED: FmResult = 0x0001_0005;
pub const FM_ERROR_PIN_DERIVE_FAILED: FmResult = 0x0001_0006;

pub const FM_ERROR_ENVELOPE_INVALID: FmResult = 0x0002_0001;
pub const FM_ERROR_UNSUPPORTED_ALGORITHM: FmResult = 0x0002_0002;
pub const FM_ERROR_RECIPIENT_NOT_FOUND: FmResult = 0x0002_0003;
pub const FM_ERROR_DECRYPT_FAILED: FmResult = 0x0002_0004;
pub const FM_ERROR_PLAINTEXT_INVALID: FmResult = 0x0002_0005;
pub const FM_ERROR_CERTIFICATE_INVALID: FmResult = 0x0002_0006;
pub const FM_ERROR_ENCRYPT_FAILED: FmResult = 0x0002_0007;
pub const FM_ERROR_RECIPIENT_INVALID: FmResult = 0x0002_0008;

/// Mirrors `FM_INIT_OPTIONS`; append-only for ABI stability.
#[repr(C)]
pub struct FmInitOptions {
    pub struct_size: u32,
    /// `const wchar_t*` — UTF-32 code units on Linux, NUL-terminated.
    pub provider_library_path: *const u32,
    pub flags: u32,
    pub reserved: u32,
}

/// Mirrors `FM_ENCRYPT_RECIPIENT` for multi-recipient encryption.
#[repr(C)]
pub struct FmEncryptRecipient {
    pub encryption_cert: *const u8,
    pub encryption_cert_length: u64,
}

extern "C" {
    pub fn FM_GetApiVersion() -> u32;

    pub fn FM_Initialize(options: *const FmInitOptions) -> FmResult;

    /// `device_list` receives '\n'-separated UTF-8 device names + trailing NUL.
    pub fn FM_EnumDevices(
        device_list: *mut std::ffi::c_char,
        device_list_size: *mut u32,
    ) -> FmResult;

    pub fn FM_ExportEncryptionCertificateBuffer(
        device_name: *const std::ffi::c_char,
        container_name: *const std::ffi::c_char,
        encryption_cert: *mut u8,
        encryption_cert_length: *mut u64,
    ) -> FmResult;

    pub fn FM_DecryptEnvelopeBuffer(
        device_name: *const std::ffi::c_char,
        container_name: *const std::ffi::c_char,
        envelope: *const u8,
        envelope_length: u64,
        plaintext: *mut u8,
        plaintext_length: *mut u64,
    ) -> FmResult;

    pub fn FM_EncryptEnvelopeBuffer(
        encryption_cert: *const u8,
        encryption_cert_length: u64,
        plaintext: *const u8,
        plaintext_length: u64,
        envelope: *mut u8,
        envelope_length: *mut u64,
    ) -> FmResult;

    pub fn FM_EncryptEnvelopeBufferEx(
        recipients: *const FmEncryptRecipient,
        recipient_count: u32,
        plaintext: *const u8,
        plaintext_length: u64,
        envelope: *mut u8,
        envelope_length: *mut u64,
    ) -> FmResult;

    /// `message_size` includes the trailing NUL.
    pub fn FM_GetLastErrorMessage(
        message: *mut std::ffi::c_char,
        message_size: *mut u32,
    ) -> FmResult;

    pub fn FM_Finalize();
}
