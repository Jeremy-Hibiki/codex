//! JSONL audit events for encrypted skill security actions.

use std::collections::HashMap;
use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::Weak;
use std::time::SystemTime;

use serde_json::json;

/// How a skill entered the encrypted-skill lifecycle. Recorded on every
/// decryption/tokenization event so periodic audits can separate explicit
/// mentions from implicit (command-driven) triggers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvocationSource {
    Explicit,
    Implicit,
}

impl InvocationSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Explicit => "explicit",
            Self::Implicit => "implicit",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditEvent {
    Decryption {
        session_id: String,
        skill_name: String,
        cache_hit: bool,
        source: InvocationSource,
    },
    Tokenization {
        session_id: String,
        skill_name: String,
        source: InvocationSource,
    },
    Rehydration {
        session_id: String,
        token_count: usize,
        skills: Vec<String>,
    },
    /// A skill decrypted through the implicit-invocation path was injected
    /// into a model request as framed content.
    ImplicitInjection {
        session_id: String,
        skill_name: String,
    },
    /// Known skill plaintext was found and redacted in a model-visible or
    /// durable surface. `skills` names the affected skills; content is never
    /// included.
    Redaction {
        session_id: String,
        skills: Vec<String>,
        surface: &'static str,
    },
    Blocked {
        session_id: String,
        tool: String,
        reason: String,
    },
    Cleanup {
        session_id: String,
        dirs_removed: usize,
        reason: String,
    },
}

impl AuditEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::Decryption { .. } => "decryption",
            Self::Tokenization { .. } => "tokenization",
            Self::Rehydration { .. } => "rehydration",
            Self::ImplicitInjection { .. } => "implicit_injection",
            Self::Redaction { .. } => "redaction",
            Self::Blocked { .. } => "blocked",
            Self::Cleanup { .. } => "cleanup",
        }
    }

    pub fn session_id(&self) -> &str {
        match self {
            Self::Decryption { session_id, .. }
            | Self::Tokenization { session_id, .. }
            | Self::Rehydration { session_id, .. }
            | Self::ImplicitInjection { session_id, .. }
            | Self::Redaction { session_id, .. }
            | Self::Blocked { session_id, .. }
            | Self::Cleanup { session_id, .. } => session_id,
        }
    }
}

/// Appends one JSONL line. Audit entries never carry skill plaintext by
/// construction.
pub fn write_event(writer: &mut impl Write, event: &AuditEvent) -> io::Result<()> {
    writeln!(writer, "{}", serialize(event))
}

pub fn serialize(event: &AuditEvent) -> String {
    let mut value = json!({
        "event": event.event_type(),
        "session_id": event.session_id(),
        "timestamp_ms": SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0),
    });
    let fields = match event {
        AuditEvent::Decryption {
            skill_name,
            cache_hit,
            source,
            ..
        } => json!({
            "skill_name": skill_name,
            "cache_hit": cache_hit,
            "source": source.as_str(),
        }),
        AuditEvent::Tokenization {
            skill_name, source, ..
        } => json!({ "skill_name": skill_name, "source": source.as_str() }),
        AuditEvent::Rehydration {
            token_count,
            skills,
            ..
        } => json!({ "token_count": token_count, "skills": skills }),
        AuditEvent::ImplicitInjection { skill_name, .. } => {
            json!({ "skill_name": skill_name })
        }
        AuditEvent::Redaction {
            skills, surface, ..
        } => {
            json!({ "skills": skills, "surface": surface })
        }
        AuditEvent::Blocked { tool, reason, .. } => {
            json!({ "tool": tool, "reason": reason })
        }
        AuditEvent::Cleanup {
            dirs_removed,
            reason,
            ..
        } => json!({ "dirs_removed": dirs_removed, "reason": reason }),
    };
    if let serde_json::Value::Object(map) = &mut value
        && let serde_json::Value::Object(fields) = fields
    {
        map.extend(fields);
    }
    value.to_string()
}

/// Destination for audit events. Implementations must never persist skill
/// plaintext (events only carry event type, session, and metadata by
/// construction).
pub trait AuditSink: Send + Sync {
    fn emit(&self, event: AuditEvent);
}

/// Append-only JSONL audit sink with size-based rotation. The active file is
/// rotated to `<path>.1` once it exceeds the size limit.
pub struct FileAuditSink {
    path: PathBuf,
    max_bytes: u64,
    writer: Mutex<std::fs::File>,
}

impl FileAuditSink {
    pub const DEFAULT_MAX_BYTES: u64 = 10 * 1024 * 1024;

    pub fn new(path: PathBuf) -> io::Result<Self> {
        Self::new_with_limit(path, Self::DEFAULT_MAX_BYTES)
    }

    pub fn new_with_limit(path: PathBuf, max_bytes: u64) -> io::Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut options = std::fs::OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let writer = options.open(&path)?;
        Ok(Self {
            path,
            max_bytes,
            writer: Mutex::new(writer),
        })
    }

    fn rotate_locked(&self, writer: &mut std::fs::File) {
        let rotated = rotated_path(&self.path);
        if let Err(error) = std::fs::remove_file(&rotated) {
            tracing::warn!(error = %error, path = %rotated.display(), "failed to remove rotated audit log");
        }
        if let Err(error) = std::fs::rename(&self.path, &rotated) {
            tracing::warn!(error = %error, path = %self.path.display(), "failed to rotate audit log");
            return;
        }
        if let Ok(fresh) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            *writer = fresh;
        } else {
            tracing::error!(path = %self.path.display(), "failed to reopen audit log after rotation");
        }
    }
}

impl AuditSink for FileAuditSink {
    fn emit(&self, event: AuditEvent) {
        let line = serialize(&event);
        let Ok(mut writer) = self.writer.lock() else {
            tracing::error!(path = %self.path.display(), "audit sink mutex poisoned; event dropped");
            return;
        };
        let over_limit = writer
            .metadata()
            .map(|metadata| metadata.len() + line.len() as u64 > self.max_bytes)
            .unwrap_or(false);
        if over_limit {
            self.rotate_locked(&mut writer);
        }
        if let Err(error) = writeln!(writer, "{line}") {
            tracing::error!(error = %error, path = %self.path.display(), "failed to write audit event");
        }
        if let Err(error) = writer.flush() {
            tracing::error!(error = %error, path = %self.path.display(), "failed to flush audit log");
        }
    }
}

/// Process-level registry of shared audit sinks, keyed by `(path, max_bytes)`.
///
/// Sessions must not open the same audit file independently: concurrent
/// rotation (`rename` to `<path>.1`) from multiple sinks can split or drop
/// events. All callers that target the same file should go through
/// [`shared_file_sink`] so writes and rotation are serialized by one writer.
/// Registry key identifying one audit file configuration.
type SharedSinkKey = (PathBuf, u64);

static SHARED_SINKS: OnceLock<Mutex<HashMap<SharedSinkKey, Weak<FileAuditSink>>>> = OnceLock::new();

/// Returns the process-wide [`FileAuditSink`] for `path` and `max_bytes`,
/// reusing the live instance when one already exists.
///
/// The key includes `max_bytes` so callers that configure a different rotation
/// limit for the same path do not silently share a writer with the wrong
/// threshold. The returned value is an `Arc<dyn AuditSink>`; the registry keeps
/// only a `Weak` reference, so the sink is reclaimed when no caller holds it.
pub fn shared_file_sink(path: PathBuf, max_bytes: u64) -> io::Result<Arc<dyn AuditSink>> {
    let map = SHARED_SINKS.get_or_init(|| Mutex::new(HashMap::new()));
    let mut map = map
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    map.retain(|_, sink| sink.strong_count() > 0);
    let key = (path.clone(), max_bytes);
    if let Some(sink) = map.get(&key).and_then(Weak::upgrade) {
        return Ok(sink as Arc<dyn AuditSink>);
    }
    let sink = Arc::new(FileAuditSink::new_with_limit(path, max_bytes)?);
    map.insert(key, Arc::downgrade(&sink));
    Ok(sink as Arc<dyn AuditSink>)
}

fn rotated_path(path: &Path) -> PathBuf {
    let mut rotated = path.as_os_str().to_owned();
    rotated.push(".1");
    PathBuf::from(rotated)
}

#[cfg(test)]
#[path = "audit_tests.rs"]
mod tests;
