## ADDED Requirements

### Requirement: Intercept direct reads of decrypted skill storage
The system SHALL block tool invocations that directly read files or directory listings under `/dev/shm/fm-agent-security/`.

#### Scenario: Direct file read is blocked
- **WHEN** a model or user issues a command that reads a file under `/dev/shm/fm-agent-security/` (for example `cat`, `less`, `head`, or `tail`)
- **THEN** the tool invocation is blocked with an explanatory message

#### Scenario: Directory probing is blocked
- **WHEN** a model or user issues a command that lists `/dev/shm/fm-agent-security/` or its subdirectories (for example `ls`, `find`)
- **THEN** the tool invocation is blocked with an explanatory message

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
Read access to decrypted skill directories SHALL be limited to text files with allowed extensions; script files SHALL be executable but their source SHALL NOT be readable through file or search tools.

#### Scenario: Read tool blocks script source
- **WHEN** the `read` tool targets a script file (for example `.py`, `.sh`, `.js`) inside a decrypted skill directory
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
