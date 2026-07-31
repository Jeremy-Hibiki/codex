//! Outbound plaintext detection (defense-in-depth).

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

#[cfg(test)]
#[path = "export_guard_tests.rs"]
mod tests;
