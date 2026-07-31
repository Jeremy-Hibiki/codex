//! Outbound plaintext detection (defense-in-depth).

use crate::paths::REDACTED_MARKER;

pub const MIN_FRAGMENT_LEN: usize = 20;

/// True when `text` contains a known plaintext, a full trimmed line of it
/// (>= 20 chars), or the 20-char prefix of such a line.
pub fn contains_known_plaintext(text: &str, known: &[&str]) -> bool {
    known
        .iter()
        .any(|plaintext| known_fragment_matches(text, plaintext))
}

/// Checks tool-argument values individually (not a serialized blob) so JSON
/// escaping cannot smuggle plaintext past matching.
pub fn args_contain_plaintext(args: &[&str], known: &[&str]) -> bool {
    args.iter().any(|arg| contains_known_plaintext(arg, known))
}

fn known_fragment_matches(text: &str, plaintext: &str) -> bool {
    if text.contains(plaintext) {
        return true;
    }
    plaintext.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.len() < MIN_FRAGMENT_LEN {
            return false;
        }
        let prefix: String = trimmed.chars().take(MIN_FRAGMENT_LEN).collect();
        text.contains(trimmed) || text.contains(&prefix)
    })
}

/// Replaces known plaintext (or long fragments of it) with `[REDACTED]`.
/// Longer matches are applied first so a full line is consumed before its
/// 20-char prefix can be re-replaced, and applying the redaction twice is a
/// no-op.
pub fn redact_known_plaintext(text: &str, known: &[&str]) -> String {
    let mut out = text.to_string();
    for plaintext in known {
        if plaintext.is_empty() {
            continue;
        }
        out = out.replace(plaintext, REDACTED_MARKER);
        for line in plaintext.lines() {
            let trimmed = line.trim();
            if trimmed.len() < MIN_FRAGMENT_LEN {
                continue;
            }
            out = out.replace(trimmed, REDACTED_MARKER);
            let prefix: String = trimmed.chars().take(MIN_FRAGMENT_LEN).collect();
            out = out.replace(&prefix, REDACTED_MARKER);
        }
    }
    out
}

#[cfg(test)]
#[path = "export_guard_tests.rs"]
mod tests;
