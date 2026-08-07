//! Prototype: tree-sitter-bash reimplementation of the guard's command
//! classification helpers.
//!
//! This module is test-only (`#[cfg(test)]`) and NOT wired into the product
//! guard. It exists to compare a real bash grammar parser against the
//! hand-written scanner in [`super::paths`] over the existing corpus and
//! bypass-shaped commands, so we can decide whether (and how) to migrate.

use tree_sitter::Node;

use super::paths;

pub(crate) fn parse(src: &str) -> Option<tree_sitter::Tree> {
    codex_shell_command::bash::try_parse_shell(src)
}

fn node_text<'a>(node: Node, src: &'a str) -> &'a str {
    &src[node.start_byte()..node.end_byte()]
}

/// Approximate shell word unquoting: strip one layer of surrounding quotes
/// and remove backslash escapes. Good enough for path-reference checks.
fn unquote(token: &str) -> String {
    let trimmed = token.trim();
    let inner = if trimmed.len() >= 2 {
        let bytes = trimmed.as_bytes();
        let first = bytes[0];
        let last = bytes[bytes.len() - 1];
        if (first == b'\'' && last == b'\'') || (first == b'"' && last == b'"') {
            &trimmed[1..trimmed.len() - 1]
        } else {
            trimmed
        }
    } else {
        trimmed
    };
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn raw_contains(cmd: &str, guarded: &[String]) -> bool {
    cmd.contains(paths::MEM_ROOT) || guarded.iter().any(|dir| cmd.contains(dir.as_str()))
}

fn collect_literal_tokens(node: Node, src: &str, out: &mut Vec<String>) {
    // Literal-bearing nodes: unquoted `word`, double-quoted `string`,
    // single-quoted `raw_string`, their `string_content`, and heredoc bodies.
    // Comments, operators, expansions and redirection syntax carry no literal
    // path and are deliberately excluded.
    if matches!(
        node.kind(),
        "word" | "string" | "raw_string" | "string_content" | "heredoc_body"
    ) {
        out.push(unquote(node_text(node, src)));
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_literal_tokens(child, src, out);
    }
}

/// Semantic path-reference detection: only literal words and heredoc bodies
/// count (comments and syntax do not). Falls back to the legacy raw-substring
/// check when the command cannot be parsed.
pub fn command_references_dir(cmd: &str, guarded: &[String]) -> bool {
    let Some(tree) = parse(cmd) else {
        return raw_contains(cmd, guarded);
    };
    if tree.root_node().has_error() {
        return raw_contains(cmd, guarded);
    }
    let mut tokens = Vec::new();
    collect_literal_tokens(tree.root_node(), cmd, &mut tokens);
    tokens.iter().any(|token| {
        paths::contains_path_prefix(token, paths::MEM_ROOT_PARENT)
            || token.contains(paths::MEM_ROOT)
            || guarded.iter().any(|dir| token.contains(dir.as_str()))
    })
}

fn is_top_level(node: Node) -> bool {
    // Check the node's ancestors only (a top-level `redirected_statement` is
    // itself allowed; its descendants are not).
    let mut current = node.parent();
    while let Some(n) = current {
        if matches!(
            n.kind(),
            "command_substitution"
                | "process_substitution"
                | "redirected_statement"
                | "file_redirect"
                | "heredoc_redirect"
                | "string"
                | "raw_string"
                | "string_content"
                | "comment"
                | "function_definition"
                | "if_statement"
                | "while_statement"
                | "until_statement"
                | "for_statement"
                | "case_statement"
                | "subshell"
                | "arithmetic_expansion"
                | "variable_expansion"
                | "simple_expansion"
        ) {
            return false;
        }
        current = n.parent();
    }
    true
}

/// Split at top-level statements using their source ranges. Bash operators
/// (`;`, `|`, `&&`, `||`, `&`, newline) are implicit in the tree's
/// `list`/`pipeline` nodes, so slicing each top-level `command`,
/// `redirected_statement` or `variable_assignment` yields the same segments
/// the legacy scanner produces by cutting at unquoted operators. Quoted,
/// escaped, commented and nested operators are never separators here.
/// Unparseable input falls back to the legacy scanner.
pub fn split_command_segments(command: &str) -> Vec<String> {
    let Some(tree) = parse(command) else {
        return paths::split_command_segments(command);
    };
    if tree.root_node().has_error() {
        return paths::split_command_segments(command);
    }

    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        let kind = node.kind();
        let inside_redirected = node
            .parent()
            .is_some_and(|parent| parent.kind() == "redirected_statement");
        if !inside_redirected
            && matches!(
                kind,
                "command" | "redirected_statement" | "variable_assignment"
            )
            && is_top_level(node)
        {
            ranges.push((node.start_byte(), node.end_byte()));
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }

    ranges.sort_unstable();
    ranges.dedup();
    let mut segments = Vec::new();
    for (start, end) in ranges {
        let segment = command[start..end].trim();
        if !segment.is_empty() {
            segments.push(segment.to_string());
        }
    }
    segments
}

fn contains_guarded(text: &str, guarded: &[String]) -> bool {
    text.contains(paths::MEM_ROOT) || guarded.iter().any(|dir| text.contains(dir.as_str()))
}

/// True when the command does NOT read a guarded path through an IO channel
/// (redirect target, command/process substitution, backticks). Plain argument
/// references (the execute-only script path) are allowed, mirroring
/// [`paths::script_execution_avoids_guarded_io`]. Falls back to the legacy
/// scanner when the command cannot be parsed.
pub fn script_execution_avoids_guarded_io(command: &str, guarded: &[String]) -> bool {
    let Some(tree) = parse(command) else {
        return paths::script_execution_avoids_guarded_io(command, guarded);
    };
    if tree.root_node().has_error() {
        return paths::script_execution_avoids_guarded_io(command, guarded);
    }
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        match node.kind() {
            "file_redirect" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if matches!(child.kind(), "word" | "string" | "raw_string") {
                        let target = unquote(node_text(child, command));
                        if contains_guarded(&target, guarded) {
                            return false;
                        }
                    }
                }
            }
            "heredoc_redirect" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if child.kind() == "heredoc_body" {
                        let body = node_text(child, command);
                        if contains_guarded(body, guarded) {
                            return false;
                        }
                    }
                }
            }
            "command_substitution" | "process_substitution"
                if contains_guarded(node_text(node, command), guarded) =>
            {
                return false;
            }
            _ => {}
        }
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            stack.push(child);
        }
    }
    true
}
