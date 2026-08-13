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
    /// The external guardrail (prompt sanitizer) flagged the user's input;
    /// `prompt` is the full flagged input, recorded for periodic review of
    /// violations and sample collection. User input is untrusted data, not
    /// skill content, so it is safe to record.
    GuardrailBlocked { session_id: String, prompt: String },
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
            Self::GuardrailBlocked { .. } => "guardrail_blocked",
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
            | Self::GuardrailBlocked { session_id, .. }
            | Self::Blocked { session_id, .. }
            | Self::Cleanup { session_id, .. } => session_id,
        }
    }
}

/// Appends one JSONL line. Audit entries never carry skill plaintext by
/// construction.
pub(crate) fn write_event(writer: &mut impl Write, event: &AuditEvent) -> io::Result<()> {
    let line = serialize(event);
    writer.write_all(line.as_bytes())?;
    writer.write_all(b"\n")
}

pub(crate) fn serialize(event: &AuditEvent) -> String {
    let timestamp_ms = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .ok()
        .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        .unwrap_or(0);
    let mut value = json!({
        "event": event.event_type(),
        "session_id": event.session_id(),
        "timestamp_ms": timestamp_ms,
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
        AuditEvent::GuardrailBlocked { prompt, .. } => {
            json!({ "prompt": prompt })
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

/// Append-only JSONL audit sink backed by `tracing-appender`'s daily rolling
/// writer. Files are named `<dir>/<stem>.<date>.<ext>`; old files are never
/// pruned. The appender appends with `O_APPEND` and the shared-writer
/// registry below serializes access across threads in this process, so
/// concurrent writers (and separate processes appending to the same file) do
/// not lose events.
#[derive(Debug)]
pub struct FileAuditSink {
    path: PathBuf,
    writer: Mutex<tracing_appender::rolling::RollingFileAppender>,
}

impl FileAuditSink {
    /// Opens an audit sink for `path`, rotating daily. The on-disk file name
    /// gets a date suffix (`<stem>.<date>.<ext>`).
    pub fn new(path: PathBuf) -> io::Result<Self> {
        let directory = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        let Some(prefix) = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
        else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("audit log path {} has no file name", path.display()),
            ));
        };
        let suffix = path
            .extension()
            .map(|ext| ext.to_string_lossy().into_owned());
        std::fs::create_dir_all(&directory)?;

        let mut builder = tracing_appender::rolling::RollingFileAppender::builder()
            .rotation(tracing_appender::rolling::Rotation::DAILY)
            .filename_prefix(&prefix);
        if let Some(suffix) = &suffix {
            builder = builder.filename_suffix(suffix);
        }
        let writer = builder.build(&directory).map_err(io::Error::other)?;
        restrict_audit_file_permissions(&directory, &prefix, suffix.as_deref())?;
        Ok(Self {
            path,
            writer: Mutex::new(writer),
        })
    }
}

impl AuditSink for FileAuditSink {
    fn emit(&self, event: AuditEvent) {
        let line = serialize(&event);
        let mut writer = match self.writer.lock() {
            Ok(writer) => writer,
            Err(poisoned) => poisoned.into_inner(),
        };
        if let Err(error) = writeln!(writer, "{line}") {
            tracing::error!(
                path = %self.path.display(),
                error = ?error,
                "failed to write audit event"
            );
        }
        if let Err(error) = writer.flush() {
            tracing::error!(
                path = %self.path.display(),
                error = ?error,
                "failed to flush audit log"
            );
        }
    }
}

/// Restricts the freshly created audit file to owner-only on Unix, matching
/// the previous hand-rolled sink's `0600` permissions.
#[cfg(unix)]
fn restrict_audit_file_permissions(
    directory: &Path,
    prefix: &str,
    suffix: Option<&str>,
) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut newest: Option<(PathBuf, std::time::SystemTime)> = None;
    let prefix_marker = format!("{prefix}.");
    let suffix_marker = suffix.map(|suffix| format!(".{suffix}"));
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with(&prefix_marker) {
            continue;
        }
        if let Some(suffix_marker) = &suffix_marker
            && !name.ends_with(suffix_marker)
        {
            continue;
        }
        let modified = entry
            .metadata()?
            .modified()
            .unwrap_or(std::time::UNIX_EPOCH);
        if newest.as_ref().is_none_or(|(_, latest)| modified > *latest) {
            newest = Some((entry.path(), modified));
        }
    }
    if let Some((path, _)) = newest {
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn restrict_audit_file_permissions(
    _directory: &Path,
    _prefix: &str,
    _suffix: Option<&str>,
) -> io::Result<()> {
    Ok(())
}

/// Process-level registry of shared audit sinks, keyed by the configured log
/// path.
///
/// Sessions must not open the same audit file independently: each
/// `RollingFileAppender` maintains its own file handle and rollover state.
/// All callers that target the same path should go through
/// [`shared_file_sink`] so writes from this process are serialized by one
/// writer.
static SHARED_SINKS: OnceLock<Mutex<HashMap<PathBuf, Weak<FileAuditSink>>>> = OnceLock::new();

/// Returns the process-wide [`FileAuditSink`] for `path`, reusing the live
/// instance when one already exists. Separate processes may append to the
/// same file: the appender opens with `O_APPEND`, and daily rotation only
/// switches to a new date-named file, so cross-process rollover cannot race.
pub fn shared_file_sink(path: PathBuf) -> io::Result<Arc<dyn AuditSink>> {
    let map = SHARED_SINKS.get_or_init(|| Mutex::new(HashMap::new()));
    {
        let mut map = map
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        map.retain(|_, sink| sink.strong_count() > 0);
        if let Some(sink) = map.get(&path).and_then(Weak::upgrade) {
            return Ok(sink as Arc<dyn AuditSink>);
        }
    }
    // Build outside the registry lock: file IO can block other callers.
    let sink = Arc::new(FileAuditSink::new(path.clone())?);
    let mut map = map
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    map.retain(|_, sink| sink.strong_count() > 0);
    if let Some(existing) = map.get(&path).and_then(Weak::upgrade) {
        return Ok(existing as Arc<dyn AuditSink>);
    }
    map.insert(path, Arc::downgrade(&sink));
    Ok(sink as Arc<dyn AuditSink>)
}

#[cfg(test)]
#[path = "audit_tests.rs"]
mod tests;
