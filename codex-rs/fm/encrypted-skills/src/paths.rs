//! Path rewriting, redaction, and command classification helpers.

use std::cmp::Reverse;
use std::path::Path;
use std::path::PathBuf;

use tree_sitter::Node;

pub const MEM_ROOT: &str = "/dev/shm/fm-agent-security";
/// Parent directory of the memory root. While a session is engaged, any
/// reference to this directory (or anything beneath it) can reach decrypted
/// storage — `find /dev/shm` lists the real plaintext tree even though it
/// never spells out [`MEM_ROOT`] — so it is guarded like the root itself.
pub const MEM_ROOT_PARENT: &str = "/dev/shm";
pub const REDACTED_MARKER: &str = "[REDACTED]";
pub const RUNNERS: [&str; 10] = [
    "python", "python3", "bash", "zsh", "sh", "node", "deno", "ruby", "perl", "pwsh",
];
pub const SCRIPT_EXTENSIONS: [&str; 7] = [".py", ".sh", ".js", ".ts", ".rb", ".pl", ".ps1"];

// ---- tree-sitter-bash command classification (primary implementation) ----

pub(crate) fn parse_shell(src: &str) -> Option<tree_sitter::Tree> {
    codex_shell_command::bash::try_parse_shell(src)
}

fn node_text<'a>(node: Node, src: &'a str) -> &'a str {
    &src[node.start_byte()..node.end_byte()]
}

/// Approximate shell word unquoting: strip one layer of surrounding quotes
/// and remove backslash escapes. Good enough for path-reference checks.
fn unquote_token(token: &str) -> String {
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

fn collect_literal_tokens(node: Node, src: &str, out: &mut Vec<String>) {
    // Literal-bearing nodes: unquoted `word`, double-quoted `string`,
    // single-quoted `raw_string`, their `string_content`, heredoc bodies, and
    // `concatenation` (word+expansion juxtaposition, e.g. `$ROOT/x`, whose
    // full text exposes variable-led path shapes).
    // Comments, operators, expansions and redirection syntax carry no literal
    // path and are deliberately excluded.
    if matches!(
        node.kind(),
        "word" | "string" | "raw_string" | "string_content" | "heredoc_body" | "concatenation"
    ) {
        out.push(unquote_token(node_text(node, src)));
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_literal_tokens(child, src, out);
    }
}

fn is_top_level(node: Node) -> bool {
    // Check the node's ancestors only (a top-level `redirected_statement` is
    // itself allowed; its descendants are not). Compound statements
    // (`if`/`while`/`until`/`for`/`case`/subshell/function bodies) are NOT
    // excluded: their inner statements qualify as segments so the guard sees
    // and cwd-tracks everything a compound can execute (H1).
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

fn contains_guarded(text: &str, guarded: &[String]) -> bool {
    if text.contains(MEM_ROOT) || guarded.iter().any(|dir| text.contains(dir.as_str())) {
        return true;
    }
    // Alias shapes (`/dev/./shm`, `/run/shm`) hold no literal guarded prefix;
    // fall back to the lexically normalized form of each path-like word.
    text.split_whitespace().any(|word| {
        let normalized = normalize_path_token(word);
        normalized != word
            && (normalized.contains(MEM_ROOT)
                || guarded.iter().any(|dir| normalized.contains(dir.as_str())))
    })
}

/// Lexically normalizes an absolute path for guarded-prefix matching: drops
/// empty and `.` components, pops `..` components, and maps the `/run/shm`
/// symlink prefix onto `/dev/shm`. Only absolute paths are normalized, so
/// relative words and URLs (`http://...`) pass through unchanged.
pub(crate) fn normalize_path_token(token: &str) -> String {
    if !token.starts_with('/') {
        return token.to_string();
    }
    let mut components: Vec<&str> = Vec::new();
    for component in token.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                components.pop();
            }
            other => components.push(other),
        }
    }
    if components.len() >= 2 && components[0] == "run" && components[1] == "shm" {
        components[0] = "dev";
        components[1] = "shm";
    }
    format!("/{}", components.join("/"))
}

/// Removes every quote character from a word, approximating bash's
/// quote-removal pass: `/dev/sh"m/fm"-agent` resolves as `/dev/shm/fm-agent`
/// even though its raw text holds no guarded prefix. Only used to widen
/// guarded matching, so aggressive stripping cannot loosen the guard.
fn strip_shell_quotes(token: &str) -> String {
    token.chars().filter(|c| *c != '"' && *c != '\'').collect()
}

/// A literal token led by a variable expansion (`$VAR/...`) cannot be
/// resolved lexically: the variable may hold a guarded path at runtime, so
/// the token is conservatively treated as a guarded reference. Tokens without
/// a `/` are not path-shaped and are never flagged.
fn is_variable_led_path(token: &str) -> bool {
    token.starts_with('$') && token.contains('/')
}

/// True when a literal word references the memory root, its parent, or any
/// known decrypted dir. Layered over [`path_references_guarded`]: bash
/// discards quotes before path resolution, so a word whose quote-stripped
/// form reaches a guarded path is also a reference (HIGH-1 ②).
/// `base` is the tracked virtual cwd: relative words are resolved against it
/// before matching (HIGH-1 ③).
fn token_references_guarded(token: &str, base: Option<&str>, dirs: &[String]) -> bool {
    if is_variable_led_path(token) {
        return true;
    }
    if path_references_guarded(token, base, dirs) {
        return true;
    }
    if token.contains('"') || token.contains('\'') {
        let stripped = strip_shell_quotes(token);
        if stripped != token
            && (is_variable_led_path(&stripped) || path_references_guarded(&stripped, base, dirs))
        {
            return true;
        }
    }
    false
}

/// True when a word (resolved against the optional virtual cwd `base` for
/// relative words) reaches a guarded path, either as a literal path or as a
/// glob anchored on a controlled directory.
fn path_references_guarded(token: &str, base: Option<&str>, dirs: &[String]) -> bool {
    if literal_token_guarded(token, dirs) {
        return true;
    }
    if has_glob_meta(token) && glob_token_references_guarded(token, dirs) {
        return true;
    }
    if let Some(base) = base
        && !token.starts_with('/')
    {
        let joined = join_lexical(base, token);
        if literal_token_guarded(&joined, dirs) {
            return true;
        }
        if has_glob_meta(&joined) && glob_token_references_guarded(&joined, dirs) {
            return true;
        }
    }
    false
}

/// Lexically joins a relative word onto a tracked absolute cwd and
/// normalizes the result (`.`/`..`/`//` collapse, `/run/shm` alias).
fn join_lexical(base: &str, relative: &str) -> String {
    normalize_path_token(&format!("{}/{}", base.trim_end_matches('/'), relative))
}

/// Literal (glob-free) guarded matching: the raw text and its lexical
/// normalization are matched against the memory root, its parent, and the
/// known decrypted dirs.
fn literal_token_guarded(token: &str, dirs: &[String]) -> bool {
    if contains_path_prefix(token, MEM_ROOT_PARENT)
        || token.contains(MEM_ROOT)
        || dirs.iter().any(|dir| token.contains(dir.as_str()))
    {
        return true;
    }
    let normalized = normalize_path_token(token);
    normalized != token
        && (contains_path_prefix(&normalized, MEM_ROOT_PARENT)
            || normalized.contains(MEM_ROOT)
            || dirs.iter().any(|dir| normalized.contains(dir.as_str())))
}

/// Shell glob/brace metacharacters that make a word's runtime path
/// resolution non-literal. `(` covers extglob forms (`@(x)`, `*(x)`, ...)
/// conservatively.
fn has_glob_meta(token: &str) -> bool {
    token.contains(['*', '?', '[', '{', '('])
}

/// True when a glob-metacharacter word can resolve at or beneath any
/// controlled directory (decrypted dirs, the memory root, registered
/// original dirs): every component of a controlled dir must be matched by
/// the word's components (in order, `*`/`?`/`[...]`/`{...}` per component),
/// with any remaining word components continuing deeper. A pure wildcard
/// with too few components (`/dev/*`) cannot reach a controlled dir and is
/// deliberately not flagged, so breadth-globs over unrelated trees keep
/// working (HIGH-1 ①).
fn glob_token_references_guarded(token: &str, dirs: &[String]) -> bool {
    let mut patterns = expand_brace_alternatives(token);
    // Glob components defeat the exact `/run/shm` alias mapping, so every
    // `/run/...` candidate also matches under `/dev/...` (only the symlinked
    // prefix is remapped; anchoring on a controlled dir is still required).
    let alias_variants: Vec<String> = patterns
        .iter()
        .filter(|p| p.starts_with("/run/"))
        .map(|p| format!("/dev/{}", &p[5..]))
        .collect();
    patterns.extend(alias_variants);
    patterns.sort();
    patterns.dedup();
    patterns.iter().any(|pattern| {
        let normalized = normalize_path_token(pattern);
        let comps: Vec<&str> = normalized.split('/').filter(|c| !c.is_empty()).collect();
        dirs.iter()
            .map(String::as_str)
            .chain(std::iter::once(MEM_ROOT))
            .any(|dir| {
                let dir_comps: Vec<&str> = dir.split('/').filter(|c| !c.is_empty()).collect();
                glob_prefix_matches(&comps, &dir_comps)
            })
    })
}

/// Anchored prefix match of a glob pattern's path components against a
/// controlled directory's components. `**` swallows any number of directory
/// components (globstar); every other component matches one component via
/// [`glob_component_matches`]. Pattern components remaining after the last
/// directory component continue deeper beneath the controlled dir (anchored
/// probing); a pattern exhausted while directory components remain cannot
/// reach the dir (HIGH-1 ①).
fn glob_prefix_matches(pattern: &[&str], dir_comps: &[&str]) -> bool {
    match pattern.split_first() {
        None => dir_comps.is_empty(),
        Some((&"**", rest)) => {
            (0..=dir_comps.len()).any(|skip| glob_prefix_matches(rest, &dir_comps[skip..]))
        }
        Some((p, rest)) => match dir_comps.split_first() {
            None => true,
            Some((d, d_rest)) => glob_component_matches(p, d) && glob_prefix_matches(rest, d_rest),
        },
    }
}

/// Expands `{a,b}` brace groups into separate candidate patterns so glob
/// matching can evaluate each expansion. A `{n..m}` range group is replaced
/// with `*` (conservative: matches any single component). Malformed groups
/// (unbalanced braces) yield the token unchanged.
fn expand_brace_alternatives(token: &str) -> Vec<String> {
    let Some((start, end)) = brace_group(token) else {
        return vec![token.to_string()];
    };
    let body = &token[start + 1..end];
    let prefix = &token[..start];
    let suffix = &token[end + 1..];
    let alternatives: Vec<String> = if !body.contains(',') && body.contains("..") {
        vec!["*".to_string()]
    } else {
        let mut depth = 0usize;
        let mut current = String::new();
        let mut parts: Vec<String> = Vec::new();
        for c in body.chars() {
            match c {
                '{' => {
                    depth += 1;
                    current.push(c);
                }
                '}' => {
                    depth = depth.saturating_sub(1);
                    current.push(c);
                }
                ',' if depth == 0 => parts.push(std::mem::take(&mut current)),
                _ => current.push(c),
            }
        }
        parts.push(current);
        parts
    };
    let mut out = Vec::new();
    for alternative in alternatives {
        let expanded = format!("{prefix}{alternative}{suffix}");
        out.extend(expand_brace_alternatives(&expanded));
    }
    out
}

/// Byte range `(start, end)` of the first balanced `{...}` group in `token`.
fn brace_group(token: &str) -> Option<(usize, usize)> {
    let start = token.find('{')?;
    let mut depth = 0usize;
    for (index, byte) in token.bytes().enumerate().skip(start) {
        match byte {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some((start, index));
                }
            }
            _ => {}
        }
    }
    None
}

/// Classic glob matching within one path component: `*` and `?` do not cross
/// `/`; `[...]` is a character class with `!`/`^` negation.
fn glob_component_matches(pattern: &str, name: &str) -> bool {
    if !has_glob_meta(pattern) {
        return pattern == name;
    }
    glob_match(pattern.as_bytes(), name.as_bytes())
}

fn glob_match(pattern: &[u8], name: &[u8]) -> bool {
    match pattern.first() {
        None => name.is_empty(),
        Some(b'*') => {
            let mut stars = 1;
            while stars < pattern.len() && pattern[stars] == b'*' {
                stars += 1;
            }
            (0..=name.len()).any(|skip| glob_match(&pattern[stars..], &name[skip..]))
        }
        Some(b'?') => !name.is_empty() && glob_match(&pattern[1..], &name[1..]),
        Some(b'[') => {
            let Some(close) = char_class_end(pattern) else {
                // Unmatched `[` is a literal character in bash.
                return name.first() == Some(&b'[') && glob_match(&pattern[1..], &name[1..]);
            };
            if name.is_empty() {
                return false;
            }
            let (body, negated) = match pattern[1] {
                b'!' | b'^' => (&pattern[2..close], true),
                _ => (&pattern[1..close], false),
            };
            char_class_matches(body, name[0]) != negated
                && glob_match(&pattern[close + 1..], &name[1..])
        }
        Some(&c) => !name.is_empty() && name[0] == c && glob_match(&pattern[1..], &name[1..]),
    }
}

/// Index of the `]` closing a `[...]` class; a `]` in first position (after
/// optional negation) is a literal per POSIX glob rules.
fn char_class_end(pattern: &[u8]) -> Option<usize> {
    let mut index = 1;
    if index < pattern.len() && (pattern[index] == b'!' || pattern[index] == b'^') {
        index += 1;
    }
    if index < pattern.len() && pattern[index] == b']' {
        index += 1;
    }
    while index < pattern.len() {
        if pattern[index] == b']' {
            return Some(index);
        }
        index += 1;
    }
    None
}

fn char_class_matches(body: &[u8], byte: u8) -> bool {
    let mut index = 0;
    while index < body.len() {
        if index + 2 < body.len() && body[index + 1] == b'-' {
            if body[index] <= byte && byte <= body[index + 2] {
                return true;
            }
            index += 3;
        } else {
            if body[index] == byte {
                return true;
            }
            index += 1;
        }
    }
    false
}

/// Collapses `/./` dot segments so the legacy fallback scanner sees the same
/// normalized path text as the tree-sitter implementation.
fn collapse_dot_segments(text: &str) -> String {
    let mut out = text.to_string();
    while out.contains("/./") {
        out = out.replace("/./", "/");
    }
    out
}

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
    let Some(tree) = parse_shell(command) else {
        return legacy_script_execution_avoids_guarded_io(command, guarded);
    };
    if tree.root_node().has_error() {
        return legacy_script_execution_avoids_guarded_io(command, guarded);
    }
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        match node.kind() {
            "file_redirect" => {
                let mut cursor = node.walk();
                for child in node.children(&mut cursor) {
                    if matches!(child.kind(), "word" | "string" | "raw_string") {
                        let target = unquote_token(node_text(child, command));
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

/// Legacy hand-written IO-channel scanner, kept as the fallback when the
/// command cannot be parsed by tree-sitter.
pub(crate) fn legacy_script_execution_avoids_guarded_io(command: &str, guarded: &[String]) -> bool {
    // Dot-segment aliases would evade the char-level prefix matching below;
    // scan the lexically collapsed text instead.
    let command = &collapse_dot_segments(command);
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
                _ if (substitution_depth > 0 || in_backtick)
                    && guarded_at(&chars, index, guarded) =>
                {
                    return false;
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
            // Output process substitution `>( ... )` runs a reader on the
            // pipe's write side; a guarded path inside it is a forbidden read.
            '>' if chars.get(index + 1) == Some(&'(') => {
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
                if contains_guarded(&target, guarded) {
                    return false;
                }
                index = end;
            }
            ')' if substitution_depth > 0 => substitution_depth -= 1,
            _ if (substitution_depth > 0 || in_backtick) && guarded_at(&chars, index, guarded) => {
                return false;
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

/// True when `text` contains `prefix` at a path-component boundary: the
/// character after the match is `/`, whitespace, a quote, an operator, or the
/// end of the text. `find /dev/shm` and `cat /dev/shm/x` match, while an
/// unrelated name such as `/dev/shmx` does not.
pub(crate) fn contains_path_prefix(text: &str, prefix: &str) -> bool {
    let bytes = text.as_bytes();
    let prefix_len = prefix.len();
    let mut search_from = 0usize;
    while let Some(offset) = text[search_from..].find(prefix) {
        let end = search_from + offset + prefix_len;
        if end == bytes.len()
            || matches!(
                bytes[end],
                b'/' | b' '
                    | b'\t'
                    | b'\n'
                    | b'\r'
                    | b'\''
                    | b'"'
                    | b'\\'
                    | b'|'
                    | b'&'
                    | b';'
                    | b'('
                    | b')'
                    | b'<'
                    | b'>'
                    | b'$'
                    | b'`'
                    | b'*'
                    | b'?'
                    | b'['
                    | b']'
                    | b'{'
                    | b'}'
                    | b','
                    | b':'
            )
        {
            return true;
        }
        search_from = end;
    }
    false
}

/// True when a command mentions the memory root, its parent directory, or any
/// known decrypted dir.
/// Substring matching means globs (`/dev/shm/.../p*/f*/SKILL.md`) and
/// cross-session directories under the shared root are also caught.
pub fn command_references_dir(cmd: &str, decrypted_dirs: &[String]) -> bool {
    command_references_dir_from(cmd, None, decrypted_dirs)
}

/// [`command_references_dir`] with an optional tracked virtual cwd: relative
/// words are resolved against `base` before matching (HIGH-1 ③). When the
/// command cannot be parsed by tree-sitter the legacy fallback runs without
/// cwd resolution — the conservative tail heuristic at the segment layer
/// still covers relative mentions there.
fn command_references_dir_from(cmd: &str, base: Option<&str>, dirs: &[String]) -> bool {
    let Some(tree) = parse_shell(cmd) else {
        return legacy_command_references_dir(cmd, dirs);
    };
    if tree.root_node().has_error() {
        return legacy_command_references_dir(cmd, dirs);
    }
    let mut tokens = Vec::new();
    collect_literal_tokens(tree.root_node(), cmd, &mut tokens);
    tokens
        .iter()
        .any(|token| token_references_guarded(token, base, dirs))
}

/// Legacy substring/prefix scanner, kept as the fallback when the command
/// cannot be parsed by tree-sitter.
pub(crate) fn legacy_command_references_dir(cmd: &str, decrypted_dirs: &[String]) -> bool {
    if contains_path_prefix(cmd, MEM_ROOT_PARENT)
        || cmd.contains(MEM_ROOT)
        || decrypted_dirs.iter().any(|dir| cmd.contains(dir.as_str()))
    {
        return true;
    }
    // Alias shapes, globs, and in-word quotes hold no literal guarded prefix;
    // check each word the same way the tree-sitter implementation does.
    // Words are unquoted first (`/dev/s\hm`, `"/dev/shm"`) so escape and
    // quote forms resolve like bash's quote-removal pass.
    cmd.split_whitespace().any(|word| {
        let unquoted = unquote_token(word);
        token_references_guarded(&unquoted, None, decrypted_dirs)
    })
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
    let Some(tree) = parse_shell(command) else {
        return legacy_split_command_segments(command);
    };
    if tree.root_node().has_error() {
        return legacy_split_command_segments(command);
    }

    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut stack = vec![tree.root_node()];
    while let Some(node) = stack.pop() {
        let kind = node.kind();
        let inside_redirected = node
            .parent()
            .is_some_and(|parent| parent.kind() == "redirected_statement");
        // Compound statements are emitted as one whole segment (in addition
        // to their inner statements, which qualify via `is_top_level`): the
        // whole-compound range keeps structural tokens (`cd /dev/shm` inside
        // an `if`, redirects spanning the compound) visible to the token
        // layer (H1).
        if !inside_redirected
            && matches!(
                kind,
                "command"
                    | "redirected_statement"
                    | "variable_assignment"
                    | "if_statement"
                    | "while_statement"
                    | "until_statement"
                    | "for_statement"
                    | "case_statement"
                    | "subshell"
                    | "function_definition"
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

/// Legacy hand-written separator scanner, kept as the fallback when the
/// command cannot be parsed by tree-sitter.
pub(crate) fn legacy_split_command_segments(command: &str) -> Vec<String> {
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

// ---- cd-chain aware segment classification (HIGH-1 ③) ----

/// The effect of a chained `cd`/`pushd` segment on the virtual working
/// directory.
enum CwdShift {
    /// Target is a lexical path; carries the new working directory.
    To(String),
    /// Target is a variable, subcommand, tilde, dirstack entry, or glob:
    /// cannot be resolved lexically. Later segments fall back to the
    /// conservative tail-component heuristic.
    Unknown,
}

/// Parses the `cd`/`pushd` target of a segment, if the segment shifts the
/// working directory. Assignment-prefixed forms (`FOO=bar cd dir`) are
/// recognized; option words (`-L`, `-P`) are skipped.
fn cwd_shift(segment: &str) -> Option<CwdShift> {
    let mut words = segment.split_whitespace().map(unquote_token);
    let verb = loop {
        let word = words.next()?;
        if word
            .split('/')
            .next()
            .is_some_and(|head| head.contains('='))
        {
            continue; // assignment prefix
        }
        break command_basename(&word).to_ascii_lowercase();
    };
    if verb != "cd" && verb != "pushd" && verb != "popd" {
        return None;
    }
    let target = words
        .find(|word| !(word.len() > 1 && word.starts_with('-')))
        .unwrap_or_default();
    if target.is_empty()
        || target == "-"
        || target.starts_with(['~', '+'])
        || target.contains(['$', '`', '('])
        || has_glob_meta(&target)
    {
        // `cd` (HOME), `cd -`/`pushd` (dirstack), and runtime glob expansion
        // all resolve outside lexical knowledge.
        return Some(CwdShift::Unknown);
    }
    Some(CwdShift::To(target))
}

/// Distinctive last path components of the controlled directories (the
/// memory root's `fm-agent-security`, decrypted dirs' `fm_skill_security_*`,
/// registered dirs' leaves), used by the conservative relative-path
/// heuristic for segments whose cwd is unknown.
fn controlled_tail_components(dirs: &[String]) -> Vec<String> {
    let mut tails: Vec<String> = Vec::new();
    for dir in dirs
        .iter()
        .map(String::as_str)
        .chain(std::iter::once(MEM_ROOT))
    {
        if let Some(tail) = dir.split('/').rfind(|c| !c.is_empty())
            && !tails.iter().any(|known| known == tail)
        {
            tails.push(tail.to_string());
        }
    }
    tails
}

/// Conservative fallback for segments whose virtual cwd is unknown: a
/// relative word mentioning a controlled directory's tail component counts
/// as a guarded reference.
fn relative_tail_guarded(segment: &str, tails: &[String]) -> bool {
    segment.split_whitespace().any(|word| {
        !word.starts_with('/') && tails.iter().any(|tail| contains_path_prefix(word, tail))
    })
}

/// cd-chain-aware per-segment guarded-reference flags: walks the segments in
/// order and maintains the virtual working directory across `cd`/`pushd`
/// segments whose targets are lexical paths, so a relative reference in a
/// later segment (`cd /dev && cd shm && cat fm-agent-security/...`) resolves
/// against the tracked cwd. Segments of commands without any `cd` are judged
/// exactly like [`command_references_dir`].
pub(crate) fn segment_guarded_flags(segments: &[String], dirs: &[String]) -> Vec<bool> {
    let mut flags = Vec::with_capacity(segments.len());
    let mut cwd: Option<String> = None;
    let mut conservative = false;
    let tails = controlled_tail_components(dirs);
    for segment in segments {
        let references = match &cwd {
            Some(base) => command_references_dir_from(segment, Some(base), dirs),
            None => {
                command_references_dir_from(segment, None, dirs)
                    || (conservative && relative_tail_guarded(segment, &tails))
            }
        };
        flags.push(references);
        match cwd_shift(segment) {
            None => {}
            Some(CwdShift::Unknown) => {
                cwd = None;
                conservative = true;
            }
            Some(CwdShift::To(target)) if target.starts_with('/') => {
                cwd = Some(normalize_path_token(&target));
            }
            Some(CwdShift::To(target)) => match &cwd {
                Some(base) => cwd = Some(join_lexical(base, &target)),
                None => conservative = true,
            },
        }
    }
    flags
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
