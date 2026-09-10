//! In-memory anonymous file helpers for transient key material.

use std::ffi::CString;
use std::fs::File;
use std::io;
use std::io::Write;
use std::path::PathBuf;

/// Creates an anonymous in-memory file (Linux `memfd`) containing `bytes`.
///
/// The file has no directory entry, so the key material is never written to
/// disk and is freed when the returned handle is dropped. On non-Linux
/// platforms this falls back to an unnamed temp file; callers that need a
/// path via [`fd_path`] should treat non-Linux as unsupported.
pub fn write_key_memfd(bytes: &[u8], name: &str) -> io::Result<File> {
    #[cfg(target_os = "linux")]
    {
        use std::os::fd::FromRawFd;

        let cname = CString::new(name)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "invalid memfd name"))?;
        // SAFETY: memfd_create takes a NUL-terminated name and flags; the
        // returned fd is owned by this process.
        let fd = unsafe { libc::memfd_create(cname.as_ptr(), 0) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let mut file = unsafe { File::from_raw_fd(fd) };
        file.write_all(bytes)?;
        Ok(file)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let mut file = tempfile::tempfile()?;
        file.write_all(bytes)?;
        Ok(file)
    }
}

/// `/proc/self/fd/N` path for an open file descriptor. Linux only.
#[cfg(target_os = "linux")]
pub fn fd_path(file: &File) -> PathBuf {
    use std::os::fd::AsRawFd;

    PathBuf::from(format!("/proc/self/fd/{}", file.as_raw_fd()))
}
