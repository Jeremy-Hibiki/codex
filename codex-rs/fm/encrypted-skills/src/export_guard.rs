//! Outbound plaintext detection (defense-in-depth).

use crate::paths::REDACTED_MARKER;
use unicode_normalization::UnicodeNormalization;

pub const MIN_FRAGMENT_LEN: usize = 20;
/// Minimum length of a quoted fragment considered for redaction.
pub const MIN_QUOTED_FRAGMENT_LEN: usize = 12;

/// True when `text` contains a known plaintext, a full trimmed line of it
/// (>= 20 chars), the 20-char prefix of such a line, or any >= 20-char
/// contiguous fragment of such a line (so `sed`/`head`-style truncation and
/// middle excerpts cannot smuggle content past matching). Both sides are
/// normalized first (case, Unicode, punctuation, whitespace, markdown
/// syntax), so formatting variations cannot hide a match.
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
    let norm_text = normalize_text(text);
    let norm_plaintext = normalize_text(plaintext);
    if norm_plaintext.is_empty() {
        return false;
    }
    if norm_text.contains(&norm_plaintext) {
        return true;
    }
    let lines: Vec<String> = plaintext
        .lines()
        .map(|line| normalize_text(line))
        .filter(|line| !line.is_empty() && line.chars().count() >= MIN_FRAGMENT_LEN)
        .collect();
    if lines.iter().any(|line| norm_text.contains(line.as_str())) {
        return true;
    }
    let chars: Vec<char> = norm_text.chars().collect();
    if chars.len() < MIN_FRAGMENT_LEN {
        return false;
    }
    (0..=chars.len() - MIN_FRAGMENT_LEN).any(|start| {
        let window: String = chars[start..start + MIN_FRAGMENT_LEN].iter().collect();
        lines.iter().any(|line| line.contains(&window))
    })
}

/// Normalizes text for fragment matching: strips markdown links and inline
/// HTML-like tags, applies Unicode NFKC + lowercase, drops punctuation and
/// symbols, and collapses whitespace runs.
pub(crate) fn normalize_text(text: &str) -> String {
    let stripped = strip_markdown_syntax(text);
    let mut out = String::with_capacity(stripped.len());
    let mut pending_space = false;
    for c in stripped.nfkc().flat_map(char::to_lowercase) {
        if c.is_alphanumeric() {
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;
            out.push(c);
        } else {
            // Whitespace and punctuation/symbols both act as token separators:
            // `The.quick.brown` normalizes to `the quick brown`, matching the
            // space-separated original.
            pending_space = true;
        }
    }
    out
}

/// Removes markdown link syntax (`[label](url)` / `![alt](url)`) and inline
/// HTML-like tags (`<b>`, `</code>`), keeping their visible text.
fn strip_markdown_syntax(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0usize;
    while index < chars.len() {
        match chars[index] {
            '[' => {
                if let Some(close) = chars[index + 1..].iter().position(|c| *c == ']') {
                    let label_end = index + 1 + close;
                    let url_start = label_end + 1;
                    if url_start < chars.len()
                        && chars[url_start] == '('
                        && let Some(paren_close) =
                            chars[url_start + 1..].iter().position(|c| *c == ')')
                    {
                        out.extend(chars[index + 1..label_end].iter());
                        index = url_start + 1 + paren_close + 1;
                        continue;
                    }
                }
                out.push(chars[index]);
                index += 1;
            }
            '<' => {
                if let Some(tag_end) = chars[index + 1..].iter().position(|c| *c == '>') {
                    let inner = &chars[index + 1..index + 1 + tag_end];
                    let looks_like_tag = !inner.is_empty()
                        && inner.iter().all(|c| {
                            c.is_ascii_alphanumeric() || *c == '/' || *c == '_' || *c == '-'
                        });
                    if looks_like_tag {
                        index += tag_end + 2;
                        continue;
                    }
                }
                out.push(chars[index]);
                index += 1;
            }
            _ => {
                out.push(chars[index]);
                index += 1;
            }
        }
    }
    out
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
    let out = redact_contiguous_fragments(&out, known);
    redact_normalized_lines(&out, known)
}

/// Whole-line fail-closed redaction after normalization: a line whose
/// normalized form contains any >= 20-char fragment of a normalized known
/// line (or exactly equals a normalized short known line) is replaced in
/// full, so case/punctuation/markdown variants cannot survive as partial
/// excerpts.
fn redact_normalized_lines(text: &str, known: &[&str]) -> String {
    let normalized_lines: Vec<String> = known
        .iter()
        .flat_map(|plaintext| plaintext.lines())
        .map(|line| normalize_text(line))
        .filter(|line| !line.is_empty())
        .collect();
    if normalized_lines.is_empty() {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let (body, suffix) = match line.strip_suffix('\n') {
            Some(body) if body.ends_with('\r') => (&body[..body.len() - 1], "\r\n"),
            Some(body) => (body, "\n"),
            None => (line, ""),
        };
        let normalized = normalize_text(body);
        let matches = if normalized.is_empty() {
            false
        } else if normalized.chars().count() < MIN_FRAGMENT_LEN {
            normalized_lines
                .iter()
                .any(|known_line| known_line == &normalized)
        } else {
            normalized_window_matches(&normalized, &normalized_lines)
        };
        if matches {
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

fn normalized_window_matches(text: &str, known_lines: &[String]) -> bool {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() < MIN_FRAGMENT_LEN {
        return false;
    }
    (0..=chars.len() - MIN_FRAGMENT_LEN).any(|start| {
        let window: String = chars[start..start + MIN_FRAGMENT_LEN].iter().collect();
        known_lines
            .iter()
            .any(|line| line.chars().count() >= MIN_FRAGMENT_LEN && line.contains(&window))
    })
}

/// Replaces any >= 20-char contiguous fragment of a known plaintext line,
/// covering truncation from the middle (`cut`/`dd`/`tail -c`) and fragmentary
/// tool output that neither forms a complete line nor starts at a line start.
fn redact_contiguous_fragments(text: &str, known: &[&str]) -> String {
    let lines: Vec<&str> = known
        .iter()
        .flat_map(|plaintext| plaintext.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty() && line.chars().count() >= MIN_FRAGMENT_LEN)
        .collect();
    if lines.is_empty() {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0usize;
    while index < chars.len() {
        let remaining = chars.len() - index;
        if remaining >= MIN_FRAGMENT_LEN {
            let window: String = chars[index..index + MIN_FRAGMENT_LEN].iter().collect();
            if lines.iter().any(|line| line.contains(&window)) {
                out.push_str(REDACTED_MARKER);
                index += MIN_FRAGMENT_LEN;
                continue;
            }
        }
        out.push(chars[index]);
        index += 1;
    }
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
