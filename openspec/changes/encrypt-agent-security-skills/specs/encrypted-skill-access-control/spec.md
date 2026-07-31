## ADDED Requirements

### Requirement: Intercept direct reads of decrypted skill storage
The system SHALL block tool invocations that directly read files or directory listings under `/dev/shm/fm-agent-security/`.

#### Scenario: Direct file read is blocked
- **WHEN** a model or user issues a command that reads a file under `/dev/shm/fm-agent-security/` (for example `cat`, `less`, `head`, or `tail`)
- **THEN** the tool invocation is blocked with an explanatory message

#### Scenario: Directory probing is blocked
- **WHEN** a model or user issues a command that lists `/dev/shm/fm-agent-security/` or its subdirectories (for example `ls`, `find`)
- **THEN** the tool invocation is blocked with an explanatory message

#### Scenario: Original-path read is not rewritten into a decrypted read
- **WHEN** a read or search command references the skill's original directory (for example `cat ~/.codex/skills/foo/SKILL.md`, `grep -r secret ~/.codex/skills/foo`)
- **THEN** the invocation is blocked instead of being rewritten to the decrypted directory

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

### Requirement: Script source isolation
Direct reads of decrypted skill directories through file-viewing or shell tools SHALL be blocked so script sources never enter the model context outside rehydration; script files SHALL remain executable. Codex has no standalone `read` tool, so this covers the actual file-viewing surface: `view_image` and shell read/search commands.

#### Scenario: File-viewing tool blocks decrypted directory access
- **WHEN** a file-viewing tool (for example `view_image`) targets any file inside a decrypted skill directory
- **THEN** the invocation is blocked

#### Scenario: Bash read commands block script source
- **WHEN** a bash command reads a script file inside a decrypted skill directory (for example `cat`, `head`, `tail`)
- **THEN** the invocation is blocked

#### Scenario: Search tools block decrypted directory access
- **WHEN** `grep` or `glob` targets a decrypted skill directory, or a bash search command (for example `grep -r`, `find`) references it
- **THEN** the invocation is blocked

#### Scenario: Script execution still allowed
- **WHEN** a command executes a script inside a decrypted skill directory (for example `python run.py`)
- **THEN** the command runs without exposing the script source

### Requirement: Outbound plaintext export blocking
Tool invocations capable of carrying plaintext out of the session (file writes, edits, network requests) SHALL be blocked when their arguments contain known decrypted skill plaintext.

#### Scenario: File write with skill plaintext is blocked
- **WHEN** a `write`, `edit`, or `apply_patch` call contains known skill plaintext in its arguments
- **THEN** the invocation is blocked with an explanatory message

#### Scenario: Network exfiltration is blocked
- **WHEN** a network tool call (for example `webfetch`, `web_search`) contains known skill plaintext in its arguments
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
