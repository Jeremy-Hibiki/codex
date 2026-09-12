use std::path::Path;

use codex_protocol::parse_command::ParsedCommand;
use codex_shell_command::parse_command::parse_command_impl;
use codex_shell_command::parse_command::tokenize_powershell_command;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathConvention;
use codex_utils_path_uri::PathUri;

use crate::SkillMetadata;

/// A skill document read or script execution identified in a shell command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImplicitSkillAccess {
    Document(PathUri),
    Script(PathUri),
}

/// Provides the indexed skill lookups used to recognize implicit invocations.
pub trait ImplicitSkillLookup {
    fn implicit_skill_for_scripts_dir(&self, path: &AbsolutePathBuf) -> Option<&SkillMetadata>;

    fn implicit_skill_for_doc_path(&self, path: &AbsolutePathBuf) -> Option<&SkillMetadata>;
}

pub fn detect_implicit_skill_invocation_for_command(
    outcome: &impl ImplicitSkillLookup,
    command: &str,
    workdir: &AbsolutePathBuf,
) -> Option<SkillMetadata> {
    let workdir = canonicalize_if_exists(workdir);
    let tokens = if PathConvention::native() == PathConvention::Windows {
        tokenize_powershell_command(command)
    } else {
        tokenize_command(command)
    };

    if let Some(candidate) = detect_skill_script_run(outcome, tokens.as_slice(), &workdir) {
        return Some(candidate);
    }

    detect_skill_doc_read(outcome, tokens.as_slice(), &workdir)
}

/// Resolves statically recognizable skill accesses without consulting the host filesystem.
pub fn implicit_skill_accesses_for_command(
    command: &str,
    workdir: &PathUri,
) -> Vec<ImplicitSkillAccess> {
    let tokens = if workdir.infer_path_convention() == Some(PathConvention::Windows) {
        tokenize_powershell_command(command)
    } else {
        tokenize_command(command)
    };
    let mut accesses = Vec::new();
    if let Some(script) = script_run_token(&tokens)
        && let Ok(path) = workdir.join(script)
    {
        accesses.push(ImplicitSkillAccess::Script(path));
    }

    for parsed in parse_command_impl(&tokens) {
        if let ParsedCommand::Read { path, .. } = parsed
            && let Some(path) = path.to_str()
            && let Ok(path) = workdir.join(path)
        {
            accesses.push(ImplicitSkillAccess::Document(path));
        }
    }

    accesses
}

fn tokenize_command(command: &str) -> Vec<String> {
    shlex::split(command)
        .unwrap_or_else(|| command.split_whitespace().map(str::to_string).collect())
}

/// Recognized script interpreters and TCL-driven EDA tools that run a script
/// file from a skill's `scripts/` directory.
///
/// EDA tools pass the script behind a tool-specific flag, e.g.
/// `vivado -mode batch -source scripts/run.tcl` or `dc_shell -f run.tcl`,
/// while `tclsh`/`wish` take it positionally.
fn script_run_token(tokens: &[String]) -> Option<&str> {
    const GENERAL_RUNNERS: [&str; 10] = [
        "python", "python3", "bash", "zsh", "sh", "node", "deno", "ruby", "perl", "pwsh",
    ];
    const TCL_RUNNERS: [&str; 16] = [
        "tclsh",
        "wish",
        "vivado",
        "vivado_hls",
        "vitis",
        "quartus_sh",
        "dc_shell",
        "dc_shell-xg",
        "dc_shell-xg-t",
        "pt_shell",
        "icc2_shell",
        "fm_shell",
        "genus",
        "innovus",
        "vsim",
        "questa",
    ];
    const SCRIPT_EXTENSIONS: [&str; 7] = [".py", ".sh", ".js", ".ts", ".rb", ".pl", ".ps1"];

    let runner_token = tokens.first()?;
    let runner = command_basename(runner_token).to_ascii_lowercase();
    let runner = runner.strip_suffix(".exe").unwrap_or(&runner);
    let runner = strip_runner_version(runner);
    if !GENERAL_RUNNERS.contains(&runner) && !TCL_RUNNERS.contains(&runner) {
        return None;
    }

    if TCL_RUNNERS.contains(&runner) {
        return tcl_script_token(&tokens[1..]);
    }

    let mut script_token = None;
    for token in tokens.iter().skip(1) {
        if token == "--" || token.starts_with('-') {
            continue;
        }
        script_token = Some(token.as_str());
        break;
    }
    let script_token = script_token?;
    if SCRIPT_EXTENSIONS
        .iter()
        .any(|extension| script_token.to_ascii_lowercase().ends_with(extension))
    {
        return Some(script_token);
    }

    None
}

/// TCL-driven EDA tools pass the script behind a flag: `-source` (Vivado/Vitis),
/// `-f` (Synopsys shells), `-t` (Quartus), `-do` (ModelSim/Questa). `tclsh`/`wish`
/// take it positionally, possibly after flag values such as `-encoding utf-8`.
fn tcl_script_token(args: &[String]) -> Option<&str> {
    for pair in args.windows(2) {
        if matches!(pair[0].as_str(), "-source" | "-f" | "-t" | "-do")
            && is_tcl_script(pair[1].as_str())
        {
            return Some(pair[1].as_str());
        }
    }
    args.iter()
        .find(|token| !token.starts_with('-') && is_tcl_script(token))
        .map(String::as_str)
}

fn is_tcl_script(token: &str) -> bool {
    token.to_ascii_lowercase().ends_with(".tcl")
}

/// Maps versioned TCL interpreter binaries such as `tclsh8.6` back to the base
/// name that identifies them as TCL runners.
fn strip_runner_version(runner: &str) -> &str {
    for base in ["tclsh", "wish"] {
        if let Some(rest) = runner.strip_prefix(base)
            && !rest.is_empty()
            && rest.chars().all(|c| c.is_ascii_digit() || c == '.')
        {
            return base;
        }
    }
    runner
}

fn detect_skill_script_run(
    outcome: &impl ImplicitSkillLookup,
    tokens: &[String],
    workdir: &AbsolutePathBuf,
) -> Option<SkillMetadata> {
    let script_token = script_run_token(tokens)?;
    let script_path = Path::new(script_token);
    let script_path = canonicalize_if_exists(&workdir.join(script_path));

    for path in script_path.ancestors() {
        if let Some(candidate) = outcome.implicit_skill_for_scripts_dir(&path) {
            return Some(candidate.clone());
        }
    }

    None
}

fn detect_skill_doc_read(
    outcome: &impl ImplicitSkillLookup,
    tokens: &[String],
    workdir: &AbsolutePathBuf,
) -> Option<SkillMetadata> {
    for command in parse_command_impl(tokens) {
        if let ParsedCommand::Read { path, .. } = command {
            let candidate_path = canonicalize_if_exists(&workdir.join(path.as_path()));
            if let Some(candidate) = outcome.implicit_skill_for_doc_path(&candidate_path) {
                return Some(candidate.clone());
            }
        }
    }

    None
}

fn command_basename(command: &str) -> String {
    Path::new(command)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(command)
        .to_string()
}

fn canonicalize_if_exists(path: &AbsolutePathBuf) -> AbsolutePathBuf {
    path.canonicalize().unwrap_or_else(|_| path.clone())
}

#[cfg(test)]
#[path = "invocation_tests.rs"]
mod tests;
