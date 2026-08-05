//! Path rewriting, redaction, and command classification helpers.

use std::cmp::Reverse;
use std::path::Path;
use std::path::PathBuf;

pub const MEM_ROOT: &str = "/dev/shm/fm-agent-security";
pub const REDACTED_MARKER: &str = "[REDACTED]";
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

/// True when the command runs a script (runner + non-flag token ending in a
/// script extension). Only the runner's first non-flag argument counts, so
/// `bash -c 'cat script.sh'` is not treated as script execution.
pub fn is_script_execution(cmd: &str) -> bool {
    let tokens: Vec<&str> = cmd.split_whitespace().collect();
    let Some(first) = tokens.first() else {
        return false;
    };
    let runner = command_basename(first).to_ascii_lowercase();
    if !RUNNERS.contains(&runner.as_str()) {
        return false;
    }
    tokens
        .iter()
        .skip(1)
        .find(|token| !token.starts_with('-'))
        .is_some_and(|target| is_script_file(unquote(target)))
}

/// True when a script-execution segment uses guarded paths only as ordinary
/// arguments. Shell I/O channels that read decrypted storage bypass the
/// "execute-only" semantic even when the segment still runs a script, so a
/// guarded path used as a redirection target, inside `$( ... )`/backticks, in
/// a here-string, or through process substitution is rejected.
pub fn script_execution_avoids_guarded_io(command: &str, guarded: &[String]) -> bool {
    let chars: Vec<char> = command.chars().collect();
    let mut index = 0usize;
    let mut single_quote = false;
    let mut double_quote = false;
    let mut substitution_depth = 0usize;
    let mut in_backtick = false;

    while index < chars.len() {
        let c = chars[index];
        if single_quote {
            if c == '\'' {
                single_quote = false;
            }
            index += 1;
            continue;
        }
        if double_quote {
            match c {
                '"' => double_quote = false,
                '`' => in_backtick = !in_backtick,
                '$' if chars.get(index + 1) == Some(&'(') => {
                    substitution_depth += 1;
                    index += 1;
                }
                ')' if substitution_depth > 0 => substitution_depth -= 1,
                _ if substitution_depth > 0 || in_backtick => {
                    if guarded_at(&chars, index, guarded) {
                        return false;
                    }
                }
                _ => {}
            }
            index += 1;
            continue;
        }
        match c {
            '\'' => single_quote = true,
            '"' => double_quote = true,
            '`' => in_backtick = !in_backtick,
            '$' if chars.get(index + 1) == Some(&'(') => {
                substitution_depth += 1;
                index += 2;
                continue;
            }
            // Process substitution `<( ... )` / `>( ... )` executes a reader
            // inside the segment; a guarded path there is a forbidden read.
            '<' if chars.get(index + 1) == Some(&'(') || chars.get(index + 1) == Some(&'>') => {
                substitution_depth += 1;
                index += 2;
                continue;
            }
            '<' | '>' => {
                let mut end = index + 1;
                while end < chars.len() && (chars[end] == '<' || chars[end] == '>') {
                    end += 1;
                }
                let target = redirect_target(&chars, end);
                if guarded.iter().any(|dir| target.contains(dir.as_str())) {
                    return false;
                }
                index = end;
            }
            ')' if substitution_depth > 0 => substitution_depth -= 1,
            _ if substitution_depth > 0 || in_backtick => {
                if guarded_at(&chars, index, guarded) {
                    return false;
                }
            }
            _ => {}
        }
        index += 1;
    }
    true
}

/// The token following a redirection operator (after optional whitespace and
/// quote handling), used to decide whether the redirect reads a guarded path.
fn redirect_target(chars: &[char], mut index: usize) -> String {
    while index < chars.len() && chars[index].is_whitespace() {
        index += 1;
    }
    let mut out = String::new();
    let mut quote: Option<char> = None;
    while index < chars.len() {
        let c = chars[index];
        if let Some(active) = quote {
            if c == active {
                break;
            }
            out.push(c);
            index += 1;
            continue;
        }
        match c {
            '\'' | '"' => {
                quote = Some(c);
                index += 1;
            }
            c if c.is_whitespace() || matches!(c, '<' | '>' | '|' | '&' | ';' | '(' | ')') => {
                break;
            }
            _ => {
                out.push(c);
                index += 1;
            }
        }
    }
    out
}

fn guarded_at(chars: &[char], index: usize, guarded: &[String]) -> bool {
    guarded.iter().any(|dir| {
        let mut offset = index;
        for expected in dir.chars() {
            match chars.get(offset) {
                Some(actual) if *actual == expected => offset += 1,
                _ => return false,
            }
        }
        true
    })
}

/// True when `path` ends in a script extension.
pub fn is_script_file(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    SCRIPT_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}

fn unquote(token: &str) -> &str {
    token.trim_matches(|c| c == '\'' || c == '"')
}

/// True when a command mentions the memory root or any known decrypted dir.
/// Substring matching means globs (`/dev/shm/.../p*/f*/SKILL.md`) and
/// cross-session directories under the shared root are also caught.
pub fn command_references_dir(cmd: &str, decrypted_dirs: &[String]) -> bool {
    if cmd.contains(MEM_ROOT) {
        return true;
    }
    decrypted_dirs.iter().any(|dir| cmd.contains(dir.as_str()))
}

/// Splits a command into independent segments at unquoted chain operators
/// (`;`, `|`, `||`, `&&`, background `&`, newline `\n`, and bash stderr pipe
/// `|&`). Each segment is judged independently by the guard so a forbidden
/// read cannot be smuggled after an allowed script execution
/// (`bash run.sh; cat SKILL.md`).
///
/// Backslash-escaped operators (`cat a\|b`) are not separators. Quote-aware:
/// operators inside single or double quotes are not separators. `&` inside a
/// redirection token (`2>&1`) is not treated as a separator.
pub fn split_command_segments(command: &str) -> Vec<String> {
    let chars: Vec<char> = command.chars().collect();
    let mut segments = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;
    let mut quote: Option<char> = None;
    while i < chars.len() {
        let c = chars[i];
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        // Backslash escape: the next char is literal, never a separator.
        // Covers `cat a\|b` and `cat a\;b` where the operator is escaped.
        if c == '\\' && i + 1 < chars.len() {
            i += 2;
            continue;
        }
        match c {
            '\'' | '"' => {
                quote = Some(c);
                i += 1;
            }
            // Bare newline is a command separator like `;`.
            '\n' => {
                push_segment(&mut segments, &chars, start, i);
                i += 1;
                start = i;
            }
            ';' | '|' => {
                // `;;` and `||` consume two chars; `|&` (bash stderr pipe)
                // also consumes two and is a separator; single char otherwise.
                let consume = if i + 1 < chars.len() && (chars[i + 1] == c || chars[i + 1] == '&') {
                    2
                } else {
                    1
                };
                push_segment(&mut segments, &chars, start, i);
                i += consume;
                start = i;
            }
            '&' => {
                if i + 1 < chars.len() && chars[i + 1] == '&' {
                    push_segment(&mut segments, &chars, start, i);
                    i += 2;
                    start = i;
                } else if i + 1 == chars.len() || chars[i + 1].is_whitespace() {
                    push_segment(&mut segments, &chars, start, i);
                    i += 1;
                    start = i;
                } else {
                    i += 1;
                }
            }
            _ => {
                i += 1;
            }
        }
    }
    push_segment(&mut segments, &chars, start, chars.len());
    segments
}

fn push_segment(segments: &mut Vec<String>, chars: &[char], start: usize, end: usize) {
    let segment: String = chars[start..end].iter().collect();
    let trimmed = segment.trim();
    if !trimmed.is_empty() {
        segments.push(trimmed.to_string());
    }
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
