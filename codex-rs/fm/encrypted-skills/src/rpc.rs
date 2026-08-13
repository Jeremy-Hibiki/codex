//! Thread-less guard helpers for app-server RPC surfaces.
//!
//! These requests do not carry a thread id, so they use the process-level
//! "any session engaged" signal. Unengaged processes behave exactly as before
//! (all checks pass through).

use std::path::Path;
use std::path::PathBuf;

use serde_json::Value;

use crate::paths;
use crate::runtime::engaged_guarded_paths;

/// True when any session in the process is currently engaged.
pub fn any_engaged() -> bool {
    crate::runtime::any_engaged()
}

fn guarded_paths() -> Vec<PathBuf> {
    engaged_guarded_paths()
}

fn guarded_strings() -> Vec<String> {
    guarded_paths()
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

/// True when `path` lies under the mem root or an engaged session's
/// decrypted directory.
pub fn is_guarded_path(path: &Path) -> bool {
    if !any_engaged() {
        return false;
    }
    guarded_paths().iter().any(|guard| path.starts_with(guard))
}

/// True when the command string references a guarded path.
pub fn command_references_guarded_path(command: &str) -> bool {
    if !any_engaged() {
        return false;
    }
    let guarded = guarded_strings();
    paths::command_references_dir(command, &guarded)
}

/// True when any string value in `value` references a guarded path.
pub fn args_reference_guarded_path(value: &Value) -> bool {
    if !any_engaged() {
        return false;
    }
    let guarded = guarded_strings();
    let mut hit = false;
    collect_string_values(value, &mut |text| {
        if paths::command_references_dir(text, &guarded) {
            hit = true;
        }
    });
    hit
}

fn collect_string_values<'a>(value: &'a Value, out: &mut impl FnMut(&'a str)) {
    match value {
        Value::String(text) => out(text),
        Value::Array(items) => {
            for item in items {
                collect_string_values(item, out);
            }
        }
        Value::Object(map) => {
            for item in map.values() {
                collect_string_values(item, out);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

#[cfg(test)]
#[path = "rpc_tests.rs"]
mod tests;
