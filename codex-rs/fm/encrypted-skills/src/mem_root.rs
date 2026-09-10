//! Decrypted skill directory layout under the memory root.

use std::fs;
use std::io;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;

use rand::RngCore;

use crate::sdk::EnvelopeError;
use crate::sdk::PackageEntry;

pub const DEFAULT_MEM_ROOT: &str = "/dev/shm/fm-agent-security";
pub const DECRYPTED_DIR_PREFIX: &str = "fm_skill_security_";
pub const PROCESS_NAMESPACE_PREFIX: &str = "p";

static INITIALIZED_ROOT: OnceLock<PathBuf> = OnceLock::new();
static INIT_ROOT_LOCK: Mutex<()> = Mutex::new(());

/// Resolves the platform-appropriate default memory root. Linux uses
/// `/dev/shm`; other platforms fall back to the process temp directory.
pub fn resolve_default_mem_root() -> PathBuf {
    #[cfg(target_os = "linux")]
    {
        PathBuf::from(DEFAULT_MEM_ROOT)
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::env::temp_dir().join("fm-agent-security")
    }
}

/// Free bytes available on the filesystem containing `path`. Returns `None`
/// on platforms where the check is unavailable (no capacity gating).
#[cfg(target_os = "linux")]
pub fn available_bytes(path: &Path) -> io::Result<Option<u64>> {
    // The memory root may not exist yet; walk up to the nearest existing
    // ancestor (statvfs needs an existing path).
    let mut current = Some(path);
    while let Some(candidate) = current {
        if candidate.exists() {
            return statvfs_free_bytes(candidate);
        }
        current = candidate.parent();
    }
    Ok(None)
}

#[cfg(target_os = "linux")]
fn statvfs_free_bytes(path: &Path) -> io::Result<Option<u64>> {
    use std::os::unix::ffi::OsStrExt;

    let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"))?;
    let mut stat = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    let result = unsafe { libc::statvfs(c_path.as_ptr(), stat.as_mut_ptr()) };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    let stat = unsafe { stat.assume_init() };
    Ok(Some(stat.f_bavail.saturating_mul(stat.f_frsize)))
}

#[cfg(not(target_os = "linux"))]
pub fn available_bytes(_path: &Path) -> io::Result<Option<u64>> {
    Ok(None)
}

/// Process-level memory-root initialization: wipes stale decrypted dirs once
/// per process, then recreates the root with mode 0700. Stale entries are
/// detected per process namespace (`p<pid>/`): namespaces whose pid is no
/// longer alive are removed, together with legacy flat `fm_skill_security_*`
/// directories. The current process's namespace is always kept.
pub fn init_mem_root_once(root: &Path) -> io::Result<()> {
    // Serialize first-time initialization so concurrent sessions cannot race
    // the stale-directory cleanup or the root recreation.
    let _guard = INIT_ROOT_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if INITIALIZED_ROOT.get().is_none() {
        init_mem_root(root)?;
        let _ = INITIALIZED_ROOT.set(root.to_path_buf());
    }
    Ok(())
}

/// Per-process namespace directory name (for example `p12345`).
pub fn process_namespace() -> String {
    format!("{PROCESS_NAMESPACE_PREFIX}{}", std::process::id())
}

/// The per-process namespace directory under `root` that this process uses.
pub fn process_namespace_dir(root: &Path) -> PathBuf {
    root.join(process_namespace())
}

/// Initializes the memory root: removes stale decrypted directories left by a
/// previous process, then recreates the root with mode 0700.
pub fn init_mem_root(root: &Path) -> io::Result<()> {
    if root.exists() {
        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let stale = if name.starts_with(PROCESS_NAMESPACE_PREFIX) {
                parse_pid(&name).is_some_and(|pid| !pid_alive(pid))
            } else {
                // Legacy flat decrypted dirs predate per-process namespaces.
                name.starts_with(DECRYPTED_DIR_PREFIX)
            };
            if stale {
                let _ = fs::remove_dir_all(entry.path());
            }
        }
    } else {
        fs::create_dir_all(root)?;
    }
    set_dir_mode_0700(root)?;
    Ok(())
}

fn parse_pid(namespace: &str) -> Option<u64> {
    namespace
        .strip_prefix(PROCESS_NAMESPACE_PREFIX)
        .and_then(|rest| rest.parse::<u64>().ok())
        .filter(|pid| *pid > 0)
}

/// Best-effort liveness check. On Linux, uses `/proc/<pid>`. On other
/// platforms we never auto-remove a namespace (stale dirs are bounded by the
/// TTL sweeps of the owning process and manual cleanup).
fn pid_alive(pid: u64) -> bool {
    #[cfg(target_os = "linux")]
    {
        Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = pid;
        true
    }
}

pub fn decrypted_dir_name(hex: &str) -> String {
    format!("{DECRYPTED_DIR_PREFIX}{hex}")
}

/// Writes decrypted package entries into `target`, rejecting Zip-Slip style
/// escapes (absolute paths or `..` traversal).
pub fn write_package_entries(entries: &[PackageEntry], target: &Path) -> Result<(), EnvelopeError> {
    // The per-process namespace parent must not be world-readable: it is
    // created here (before the leaf) and holds the decrypted directory names.
    if let Some(namespace) = target.parent() {
        fs::create_dir_all(namespace)?;
        set_dir_mode_0700(namespace)?;
    }
    fs::create_dir_all(target)?;
    set_dir_mode_0700(target)?;
    for entry in entries {
        validate_rel_path(&entry.rel_path)?;
        let dest = target.join(&entry.rel_path);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
            set_dir_mode_0700(parent)?;
        }
        fs::write(dest, &entry.contents)?;
    }
    Ok(())
}

fn validate_rel_path(rel: &Path) -> Result<(), EnvelopeError> {
    let bad = rel.is_absolute()
        || rel.components().any(|c| {
            matches!(
                c,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        });
    if bad {
        return Err(EnvelopeError::InvalidEntry {
            path: rel.to_string_lossy().into_owned(),
            reason: "entry escapes the decrypted directory".into(),
        });
    }
    Ok(())
}

/// Secure wipe: removes the whole decrypted tree.
///
/// On a tmpfs memory root (Linux `/dev/shm`), files live in RAM — overwriting
/// them with random bytes is forensic theatre and can only push plaintext
/// pages into swap. We therefore skip the overwrite and go straight to
/// `remove_dir_all`. On non-tmpfs roots (the non-Linux `temp_dir()` fallback
/// or a custom disk-backed root) we overwrite regular files first so a
/// recoverable medium does not retain plaintext.
pub fn secure_wipe(dir: &Path) -> io::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    if !is_tmpfs_root(dir) {
        overwrite_tree_files_random(dir)?;
    }
    fs::remove_dir_all(dir)
}

/// True when `dir` is on a Linux tmpfs (`/dev/shm`), where plaintext pages
/// are RAM and overwrite-before-delete is pointless.
fn is_tmpfs_root(dir: &Path) -> bool {
    #[cfg(target_os = "linux")]
    {
        // Walk up to the nearest existing ancestor and stat its device.
        let mut probe = dir;
        loop {
            if probe.exists() {
                break;
            }
            match probe.parent() {
                Some(parent) => probe = parent,
                None => return false,
            }
        }
        is_dev_shm(probe)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = dir;
        false
    }
}

/// Overwrites every regular file under `dir` with random bytes, leaving the
/// directory structure intact for the caller to remove.
fn overwrite_tree_files_random(dir: &Path) -> io::Result<()> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        for entry in fs::read_dir(&current)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                stack.push(entry.path());
            } else {
                files.push(entry.path());
            }
        }
    }
    for file in files {
        overwrite_with_random(&file)?;
        fs::remove_file(file)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn is_dev_shm(path: &Path) -> bool {
    // `/dev/shm` is a tmpfs mount; detect by checking whether the canonical
    // path starts with the default memory root prefix.
    path.starts_with(DEFAULT_MEM_ROOT)
}

#[cfg(not(target_os = "linux"))]
fn is_dev_shm(_path: &Path) -> bool {
    false
}

fn overwrite_with_random(path: &Path) -> io::Result<()> {
    use std::io::Write;

    let len = fs::metadata(path)?.len();
    let mut file = fs::OpenOptions::new().write(true).open(path)?;
    let mut buf = [0u8; 4096];
    let mut remaining = len;
    while remaining > 0 {
        let chunk = remaining.min(buf.len() as u64) as usize;
        rand::rng().fill_bytes(&mut buf[..chunk]);
        file.write_all(&buf[..chunk])?;
        remaining -= chunk as u64;
    }
    file.sync_all()?;
    Ok(())
}

#[cfg(unix)]
fn set_dir_mode_0700(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
}

#[cfg(not(unix))]
fn set_dir_mode_0700(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
#[path = "mem_root_tests.rs"]
mod tests;
