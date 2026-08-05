//! Outbound plaintext detection (defense-in-depth).

use crate::paths::REDACTED_MARKER;

pub const MIN_FRAGMENT_LEN: usize = 20;
/// Minimum length of a quoted fragment considered for redaction.
pub const MIN_QUOTED_FRAGMENT_LEN: usize = 12;

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
    let mut short_lines: Vec<&str> = Vec::new();
    let mut out = text.to_string();
    for plaintext in known {
        if plaintext.is_empty() {
            continue;
        }
        out = out.replace(plaintext, REDACTED_MARKER);
        for line in plaintext.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.len() < MIN_FRAGMENT_LEN {
                if !short_lines.contains(&trimmed) {
                    short_lines.push(trimmed);
                }
                continue;
            }
            out = out.replace(trimmed, REDACTED_MARKER);
            let prefix: String = trimmed.chars().take(MIN_FRAGMENT_LEN).collect();
            out = out.replace(&prefix, REDACTED_MARKER);
        }
    }
    if !short_lines.is_empty() {
        out = redact_complete_short_lines(&out, &short_lines);
    }
    out = redact_quoted_fragments(&out, known);
    out
}

/// Replaces short skill lines only when they appear as a complete line in the
/// reply, so a quoted secret line is redacted without false positives on the
/// same fragment embedded inside a sentence.
fn redact_complete_short_lines(text: &str, short_lines: &[&str]) -> String {
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let (body, suffix) = match line.strip_suffix('\n') {
            Some(body) if body.ends_with('\r') => (&body[..body.len() - 1], "\r\n"),
            Some(body) => (body, "\n"),
            None => (line, ""),
        };
        let trimmed = body.trim();
        if !trimmed.is_empty() && short_lines.contains(&trimmed) {
            let leading_len = body.len() - body.trim_start().len();
            out.push_str(&body[..leading_len]);
            out.push_str(REDACTED_MARKER);
            out.push_str(suffix);
        } else {
            out.push_str(line);
        }
    }
    out
}

/// Redacts quoted fragments that quote a known plaintext line from the middle
/// (for example 「long sensitive line」), covering the case where the model
/// quotes a line segment that is neither a line start nor a complete line.
/// Quote styles covered: `"…"`, `'…'`, `「…」`, `“…”`.
fn redact_quoted_fragments(text: &str, known: &[&str]) -> String {
    const PAIRS: [(char, char); 4] = [('"', '"'), ('\'', '\''), ('「', '」'), ('“', '”')];
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let open = chars[i];
        if let Some((_, close)) = PAIRS.iter().find(|(opener, _)| *opener == open)
            && let Some(offset) = chars[i + 1..].iter().position(|c| *c == *close)
        {
            let end = i + 1 + offset;
            let content: String = chars[i + 1..end].iter().collect();
            if quoted_fragment_matches(&content, known) {
                out.push(open);
                out.push_str(REDACTED_MARKER);
                out.push(*close);
                i = end + 1;
                continue;
            }
        }
        out.push(open);
        i += 1;
    }
    out
}

fn quoted_fragment_matches(content: &str, known: &[&str]) -> bool {
    if content.chars().count() < MIN_QUOTED_FRAGMENT_LEN {
        return false;
    }
    for plaintext in known {
        for line in plaintext.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let prefix: String = trimmed.chars().take(MIN_QUOTED_FRAGMENT_LEN).collect();
            if content.contains(&prefix) || trimmed.contains(content) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
#[path = "export_guard_tests.rs"]
mod tests;
