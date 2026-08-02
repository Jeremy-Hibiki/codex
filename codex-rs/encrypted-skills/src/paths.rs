//! Path rewriting, redaction, and command classification helpers.

use std::cmp::Reverse;
use std::path::Path;
use std::path::PathBuf;

pub const MEM_ROOT: &str = "/dev/shm/fm-agent-security";
pub const REDACTED_MARKER: &str = "[REDACTED]";
pub const TEXT_EXTENSIONS: [&str; 3] = [".md", ".txt", ".markdown"];
pub const READ_COMMANDS: [&str; 14] = [
    "cat", "head", "tail", "less", "more", "tac", "nl", "bat", "batcat", "xxd", "od", "hexdump",
    "strings", "base64",
];
pub const SEARCH_COMMANDS: [&str; 12] = [
    "grep", "rg", "sed", "awk", "find", "ls", "cut", "tr", "paste", "column", "shuf", "uniq",
];
pub const RUNNERS: [&str; 10] = [
    "python", "python3", "bash", "zsh", "sh", "node", "deno", "ruby", "perl", "pwsh",
];
pub const SCRIPT_EXTENSIONS: [&str; 7] = [".py", ".sh", ".js", ".ts", ".rb", ".pl", ".ps1"];

/// Replaces every occurrence of each original directory with its decrypted
/// counterpart, applying longest originals first to avoid prefix collisions.
pub fn rewrite_skill_paths(text: &str, mappings: &[(PathBuf, PathBuf)]) -> String {
    let mut sorted: Vec<&(PathBuf, PathBuf)> = mappings.iter().collect();
    sorted.sort_by_key(|(from, _)| Reverse(from.as_os_str().len()));
    let mut out = text.to_string();
    for (from, to) in sorted {
        out = out.replace(
            from.to_string_lossy().as_ref(),
            to.to_string_lossy().as_ref(),
        );
    }
    out
}

/// Replaces the decrypted memory-root path (and any path segment following it)
/// with `[REDACTED]`.
pub fn redact_decrypted_paths(text: &str) -> String {
    redact_path_prefix(text, MEM_ROOT)
}

/// Replaces `prefix` (and any path segment following it) with `[REDACTED]`.
pub fn redact_path_prefix(text: &str, prefix: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(index) = rest.find(prefix) {
        out.push_str(&rest[..index]);
        let tail = &rest[index..];
        let mut end = prefix.len();
        let bytes = tail.as_bytes();
        while end < bytes.len() {
            let byte = bytes[end];
            if byte.is_ascii_whitespace() || matches!(byte, b'"' | b'\'' | b']' | b')' | b'>') {
                break;
            }
            end += 1;
        }
        out.push_str(REDACTED_MARKER);
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

pub fn is_allowed_text_file(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    TEXT_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}

pub fn is_read_command(cmd: &str) -> bool {
    command_basenames(cmd).any(|name| READ_COMMANDS.contains(&name))
}

pub fn is_search_command(cmd: &str) -> bool {
    command_basenames(cmd).any(|name| SEARCH_COMMANDS.contains(&name))
}

/// True when the command runs a script (runner + non-flag token ending in a
/// script extension).
pub fn is_script_execution(cmd: &str) -> bool {
    let tokens: Vec<&str> = cmd.split_whitespace().collect();
    let Some(first) = tokens.first() else {
        return false;
    };
    let runner = command_basename(first).to_ascii_lowercase();
    if !RUNNERS.contains(&runner.as_str()) {
        return false;
    }
    tokens.iter().skip(1).any(|token| {
        if token.starts_with('-') {
            return false;
        }
        let lower = token.to_ascii_lowercase();
        SCRIPT_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
    })
}

/// True when `path` ends in a script extension.
pub fn is_script_file(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    SCRIPT_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}

/// Returns non-flag, non-redirection file-like tokens in a command, excluding
/// the command name itself.
pub fn file_targets(cmd: &str) -> Vec<&str> {
    let tokens: Vec<&str> = cmd.split_whitespace().collect();
    let mut out = Vec::new();
    let mut skip_next = false;
    for (index, token) in tokens.iter().enumerate() {
        if skip_next {
            skip_next = false;
            continue;
        }
        match *token {
            ">" | ">>" | "<" | "2>" | "2>>" | "1>" | "1>>" | "&>" | "&>>" => {
                skip_next = true;
                continue;
            }
            "|" | "||" | "&&" | ";" => continue,
            _ => {}
        }
        if index == 0 || token.starts_with('-') {
            continue;
        }
        // Embedded redirections like `2>/dev/null` carry no file target of
        // interest for script detection.
        if token.contains('>')
            && (token.starts_with('>')
                || token
                    .as_bytes()
                    .first()
                    .is_some_and(|byte| byte.is_ascii_digit() || *byte == b'&'))
        {
            continue;
        }
        out.push(*token);
    }
    out
}

/// True when a read/search command targets at least one script file.
pub fn command_targets_script(cmd: &str) -> bool {
    file_targets(cmd)
        .iter()
        .any(|target| is_script_file(target))
}

/// True when the command is a recursive grep/rg search (rg recurses by
/// default).
pub fn is_recursive_search(cmd: &str) -> bool {
    if command_basenames(cmd).any(|name| name == "rg") {
        return true;
    }
    cmd.split_whitespace().any(|token| {
        (token == "-r" || token == "-R" || token == "--recursive")
            || ((token.starts_with("-r") || token.starts_with("-R")) && token.len() > 2)
    })
}

/// Appends script-extension exclusions so a recursive grep/rg never emits
/// script source lines.
pub fn inject_script_exclusions(cmd: &str) -> String {
    let exclusions: Vec<String> = SCRIPT_EXTENSIONS
        .iter()
        .map(|ext| format!("--exclude='*{ext}'"))
        .collect();
    format!("{cmd} {}", exclusions.join(" "))
}

/// True when a command mentions the memory root or any known decrypted dir.
pub fn command_references_dir(cmd: &str, decrypted_dirs: &[String]) -> bool {
    if cmd.contains(MEM_ROOT) {
        return true;
    }
    decrypted_dirs.iter().any(|dir| cmd.contains(dir.as_str()))
}

fn command_basenames(cmd: &str) -> impl Iterator<Item = &str> {
    cmd.split_whitespace()
        .map(command_basename)
        .filter(|name| !name.is_empty())
}

fn command_basename(token: &str) -> &str {
    Path::new(token)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(token)
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
