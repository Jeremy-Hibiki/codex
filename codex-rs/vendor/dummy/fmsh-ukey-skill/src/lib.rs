//! Placeholder for the internal `fmsh-ukey-skill` crate of `fmsh-ukey-lib`
//! (internal GitLab 192.168.131.126:8089).
//!
//! Only [`detect_mode`] is consumed downstream (by `fm-encrypted-skills`).
//! The stub reimplements the documented upstream semantics without the
//! packaging toolchain:
//!
//! 1. `SKILL.md` YAML frontmatter `metadata.encryption.mode` wins
//!    (`mock` / `software` / `ukey` / `ukey_two_phase`, `-` also accepted);
//! 2. otherwise the presence of a `key.enc` file selects
//!    [`Mode::UkeyTwoPhase`];
//! 3. otherwise [`Mode::Ukey`].
//!
//! Unknown or missing modes fall back per rule 2/3, matching the upstream
//! "missing backends fail closed; unknown/missing modes fall back to UKey"
//! behavior.

use std::path::Path;

/// Decryption mode of an encrypted skill package.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// `.zip.enc` is a plain ZIP renamed (upstream simulation mode).
    Mock,
    /// Software digital envelope (HPKE or SM2/SM4 CMS).
    Software,
    /// CMS SM2/SM4 envelope through the FMSH UKey SDK.
    Ukey,
    /// UKey-unwrapped per-skill key, packages decrypt in software AES-GCM.
    UkeyTwoPhase,
}

impl Mode {
    /// Canonical mode string used in diagnostics.
    pub fn as_str(&self) -> &'static str {
        match self {
            Mode::Mock => "mock",
            Mode::Software => "software",
            Mode::Ukey => "ukey",
            Mode::UkeyTwoPhase => "ukey_two_phase",
        }
    }
}

/// Detect the decryption mode for a skill package directory.
pub fn detect_mode(skill_dir: &Path) -> Mode {
    let skill_md = skill_dir.join("SKILL.md");
    if let Ok(contents) = std::fs::read_to_string(&skill_md) {
        if let Some(mode) = frontmatter_mode(&contents) {
            return mode;
        }
    }
    if skill_dir.join("key.enc").is_file() {
        Mode::UkeyTwoPhase
    } else {
        Mode::Ukey
    }
}

/// Parse `metadata.encryption.mode` out of a SKILL.md YAML frontmatter
/// block. Intentionally minimal: matches the indented `mode: <value>` entry
/// inside the `encryption:` mapping of the standard skill frontmatter, which
/// is all the upstream packaging format specifies.
fn frontmatter_mode(contents: &str) -> Option<Mode> {
    let trimmed = contents.trim_start();
    let rest = trimmed.strip_prefix("---")?;
    let end = rest.find("\n---")?;
    let frontmatter = &rest[..end];
    let mut in_encryption = false;
    for line in frontmatter.lines() {
        let indent = line.len() - line.trim_start().len();
        let line = line.trim();
        if let Some(value) = line.strip_prefix("encryption:") {
            in_encryption = value.trim().is_empty();
            continue;
        }
        if in_encryption {
            if indent == 0 {
                in_encryption = false;
                continue;
            }
            if let Some(value) = line.strip_prefix("mode:") {
                return parse_mode(value.trim());
            }
        }
    }
    None
}

fn parse_mode(raw: &str) -> Option<Mode> {
    match raw {
        "mock" => Some(Mode::Mock),
        "software" => Some(Mode::Software),
        "ukey" => Some(Mode::Ukey),
        "ukey_two_phase" | "ukey-two-phase" => Some(Mode::UkeyTwoPhase),
        // Unknown modes fall back to the key.enc/UKey heuristic, per the
        // upstream semantics; returning None re-triggers that path.
        _ => None,
    }
}

#[cfg(test)]
#[path = "detect_mode_tests.rs"]
mod tests;
