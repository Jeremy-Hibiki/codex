## ADDED Requirements

### Requirement: Execute only access to decrypted storage
The system SHALL allow only script execution against decrypted skill storage. Every other reference to the memory root or any decrypted directory — reading text or script files, copying, redirecting, piping, listing, searching, or globbing — SHALL be blocked, regardless of the command name. This is a semantic gate, not a command-name allowlist, so it cannot be bypassed by `cp SKILL.md /tmp`, redirection, or commands outside a curated list.

The check operates on each segment of the command after splitting at unquoted chain operators (`;`, `|`, `&&`, `||`, and background `&`), so a forbidden read cannot be smuggled after an allowed execution. The check runs against the path-rewritten command (original skill directories already rewritten to their decrypted counterparts) so original-path and decrypted-path references are judged on the same canonical view.

#### Scenario: Reading a text file (including SKILL.md) is blocked
- **WHEN** a command reads any file under the memory root or a decrypted directory, including text files such as `SKILL.md`, `notes.md`, or `references/*.md`
- **THEN** the invocation is blocked with an explanatory message

#### Scenario: Directory probing and listing are blocked
- **WHEN** a command lists the memory root or any decrypted directory (for example `ls`, `find`, `tree`, `eza`, `lsd`, `fd`)
- **THEN** the invocation is blocked

#### Scenario: Copying or redirecting plaintext out of storage is blocked
- **WHEN** a command copies, redirects, or encodes a file from decrypted storage to an outside location (for example `cp <dir>/SKILL.md /tmp/leak.md`, `cat <dir>/SKILL.md > /tmp/leak.md`, `base64 <dir>/SKILL.md`)
- **THEN** the invocation is blocked

#### Scenario: Original-path read is not rewritten into a decrypted read
- **WHEN** a read or search command references the skill's original directory (for example `cat ~/.codex/skills/foo/SKILL.md`, `grep -r secret ~/.codex/skills/foo`)
- **THEN** the invocation is blocked instead of being rewritten to the decrypted directory

#### Scenario: Glob probing of decrypted storage is blocked
- **WHEN** a command uses shell globbing to reach decrypted storage without knowing the exact path (for example `cat /dev/shm/fm-agent-security/p*/f*/SKILL.md`)
- **THEN** the invocation is blocked

#### Scenario: Cross-session directory access is blocked
- **WHEN** a command references a decrypted directory that belongs to another session but still lives under the shared memory root
- **THEN** the invocation is blocked

#### Scenario: Chain-operator smuggled read is blocked
- **WHEN** a command chains a forbidden read after an allowed script execution via `;`, `&&`, `||`, `|`, or background `&` (for example `bash <dir>/scripts/build.sh; cat <dir>/SKILL.md`)
- **THEN** the invocation is blocked, because each chain segment is judged independently

#### Scenario: File-viewing tool blocks decrypted directory access
- **WHEN** a file-viewing tool (for example `view_image`) targets any file inside a decrypted skill directory
- **THEN** the invocation is blocked

### Requirement: Allow script execution from decrypted storage
The system SHALL allow executing scripts located under `/dev/shm/fm-agent-security/` without exposing their source content.

#### Scenario: Executing a decrypted script
- **WHEN** a command executes a script under `/dev/shm/fm-agent-security/` (for example `bash .../scripts/build.sh`)
- **THEN** the command runs normally

### Requirement: Tool-argument path rewriting to decrypted storage
Before executing a tool whose arguments reference a decrypted skill's original directory or skill-relative paths, the system SHALL rewrite those references to the session's decrypted directory.

#### Scenario: Shell command with original skill path is rewritten
- **WHEN** a shell command references the skill's original directory (for example `bash ~/.codex/skills/foo/scripts/build.sh`)
- **THEN** the command is rewritten to the session's decrypted directory before execution
- **AND** the real script under `/dev/shm/fm-agent-security/` executes

#### Scenario: File tool with relative resource path is rewritten
- **WHEN** a file tool references a package file relative to the skill (for example `resources/config.json`)
- **THEN** the path is resolved against the skill's original directory and rewritten to the decrypted directory before the tool runs

#### Scenario: Unrelated paths pass through
- **WHEN** a tool invocation does not reference any known skill directory
- **THEN** the arguments are left unchanged

### Requirement: Decrypted path redaction in tool output
Tool outputs containing the decrypted storage path SHALL be redacted before the output enters the model context or is persisted.

#### Scenario: Script prints its own decrypted path
- **WHEN** a tool output contains a `/dev/shm/fm-agent-security/...` path string
- **THEN** the path string is replaced with a redaction marker
- **AND** the model never observes the decrypted path in tool results

#### Scenario: Script references relative resources
- **WHEN** an executed script or the `SKILL.md` document references resources via relative paths within its decrypted directory
- **THEN** the references resolve inside the decrypted directory without requiring the absolute storage path in model-visible context


### Requirement: Outbound plaintext export blocking
Tool invocations capable of carrying plaintext out of the session (file writes, edits, network requests) SHALL be blocked when their arguments contain known decrypted skill plaintext.

#### Scenario: File write with skill plaintext is blocked
- **WHEN** a `write`, `edit`, or `apply_patch` call contains known skill plaintext in its arguments
- **THEN** the invocation is blocked with an explanatory message

#### Scenario: Network exfiltration is blocked
- **WHEN** a network tool call (for example `webfetch`, `web_search`) contains known skill plaintext in its arguments
- **THEN** the invocation is blocked with an explanatory message

#### Scenario: MCP and extension tool arguments are checked
- **WHEN** an MCP, extension, or any other non-shell tool call contains known skill plaintext in any string argument
- **THEN** the invocation is blocked with an explanatory message

#### Scenario: Normal file operations pass through
- **WHEN** a file or network tool call does not contain known skill plaintext
- **THEN** the invocation proceeds normally

### Requirement: Persisted model replies are redacted
The system SHALL hard-enforce the skill output policy at the durable history boundary: assistant replies and plaintext inter-agent messages that quote known skill plaintext SHALL be redacted before they are written to in-memory history, rollout, or the client stream.

#### Scenario: Assistant reply quoting a skill line is redacted
- **WHEN** an assistant message contains a known skill line of at least 20 characters or the 20-character prefix of such a line
- **THEN** the quoted fragment is replaced with the redaction marker in the recorded, persisted, and streamed message

#### Scenario: Assistant reply quoting a short skill line is redacted
- **WHEN** an assistant message contains a shorter skill line (for example `api_key=abc`) as a complete line
- **THEN** that line is replaced with the redaction marker

#### Scenario: Short fragment embedded in a sentence is preserved
- **WHEN** an assistant message contains the same short fragment embedded inside a sentence (not as a complete line)
- **THEN** the text is left unchanged

#### Scenario: User messages are never redacted
- **WHEN** a recorded message has a role other than assistant
- **THEN** the content is left unchanged

#### Scenario: Every persistence surface of a quoted reply is redacted
- **WHEN** the model reply quotes known skill plaintext
- **THEN** the recorded response item, derived turn item (started/completed events), reasoning text, and `last_agent_message` in the task-complete event all contain the redaction marker instead of the plaintext
