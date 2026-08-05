//! JSONL audit events for encrypted skill security actions.

use std::fmt::Write as _;
use std::io;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuditEvent {
    Decryption {
        session_id: String,
        skill_name: String,
        cache_hit: bool,
    },
    Tokenization {
        session_id: String,
        skill_name: String,
    },
    Rehydration {
        session_id: String,
        token_count: usize,
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
            Self::Blocked { .. } => "blocked",
            Self::Cleanup { .. } => "cleanup",
        }
    }

    pub fn session_id(&self) -> &str {
        match self {
            Self::Decryption { session_id, .. }
            | Self::Tokenization { session_id, .. }
            | Self::Rehydration { session_id, .. }
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
    let mut out = String::new();
    let _ = write!(
        out,
        "{{\"event\":\"{}\",\"session_id\":\"{}\"",
        event.event_type(),
        event.session_id()
    );
    match event {
        AuditEvent::Decryption {
            skill_name,
            cache_hit,
            ..
        } => {
            let _ = write!(
                out,
                ",\"skill_name\":\"{skill_name}\",\"cache_hit\":{cache_hit}"
            );
        }
        AuditEvent::Tokenization { skill_name, .. } => {
            let _ = write!(out, ",\"skill_name\":\"{skill_name}\"");
        }
        AuditEvent::Rehydration { token_count, .. } => {
            let _ = write!(out, ",\"token_count\":{token_count}");
        }
        AuditEvent::Blocked { tool, reason, .. } => {
            let _ = write!(out, ",\"tool\":\"{tool}\",\"reason\":\"{reason}\"");
        }
        AuditEvent::Cleanup {
            dirs_removed,
            reason,
            ..
        } => {
            let _ = write!(
                out,
                ",\"dirs_removed\":{dirs_removed},\"reason\":\"{reason}\""
            );
        }
    }
    let _ = write!(out, "}}");
    out
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
        let writer = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        Ok(Self {
            path,
            max_bytes,
            writer: Mutex::new(writer),
        })
    }

    fn rotate_locked(&self, writer: &mut std::fs::File) {
        let rotated = rotated_path(&self.path);
        let _ = std::fs::remove_file(&rotated);
        let _ = std::fs::rename(&self.path, &rotated);
        if let Ok(fresh) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            *writer = fresh;
        }
    }
}

impl AuditSink for FileAuditSink {
    fn emit(&self, event: AuditEvent) {
        let line = serialize(&event);
        let Ok(mut writer) = self.writer.lock() else {
            return;
        };
        let over_limit = writer
            .metadata()
            .map(|metadata| metadata.len() + line.len() as u64 > self.max_bytes)
            .unwrap_or(false);
        if over_limit {
            self.rotate_locked(&mut writer);
        }
        let _ = writeln!(writer, "{line}");
        let _ = writer.flush();
    }
}

fn rotated_path(path: &Path) -> PathBuf {
    let mut rotated = path.as_os_str().to_owned();
    rotated.push(".1");
    PathBuf::from(rotated)
}

#[cfg(test)]
#[path = "audit_tests.rs"]
mod tests;
