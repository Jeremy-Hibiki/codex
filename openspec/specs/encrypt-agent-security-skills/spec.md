# encrypt-agent-security-skills

## Purpose

加密 Skill 的数字信封解密、token 化注入、execute-only 访问控制与 fork 隔离（早期实现基线）。

## ADDED Requirements

### Requirement: Encrypted skill decryption on mention
When an encrypted skill is mentioned, the system SHALL detect the encryption marker in the SKILL.md frontmatter (`metadata.encrypted` / `metadata.encryption`) or the presence of the `<name>.zip.enc` package in the skill directory, and decrypt the package through the digital envelope SDK into a decrypted directory at `/dev/shm/fm-agent-security/fm_skill_security_<hex>/` tracked by a session-scoped registry before injecting it into context.

#### Scenario: Encrypted skill mentioned in a turn
- **WHEN** a user message mentions an encrypted skill name
- **THEN** the system decrypts the skill content to a directory at `/dev/shm/fm-agent-security/fm_skill_security_<hex>/`
- **AND** the decrypted directory contains the real `SKILL.md` plus any `scripts/`, `agents/`, `references/`, and `templates/` shipped inside the package
- **AND** the mapping from the session to the decrypted directory is recorded in the session registry

#### Scenario: Encryption status is known at trigger time
- **WHEN** a skill is resolved as mentioned during the turn
- **THEN** the resolved skill metadata already carries its encryption status and package name from frontmatter parsing
- **AND** the injection path branches to decryption for encrypted skills without re-reading the SKILL.md file

#### Scenario: Disk stub is not injected
- **WHEN** a skill directory contains an encrypted package marker
- **THEN** the stub `SKILL.md` on disk is never injected as skill content
- **AND** only the decrypted package content is used for injection and rehydration

#### Scenario: Re-mention within TTL reuses decrypted content
- **WHEN** a skill already decrypted in this session is triggered again within its idle TTL
- **THEN** the existing decrypted directory is reused
- **AND** the SDK is not invoked again for decryption

#### Scenario: Decryption failure
- **WHEN** the SDK fails to decrypt the skill content
- **THEN** the system does not inject any skill content
- **AND** the system records a warning describing the failure

### Requirement: Token-only context injection
The system SHALL inject encrypted skill content into the model context as a token placeholder only, never as plaintext.

#### Scenario: Context contains only the token placeholder
- **WHEN** an encrypted skill is injected into context
- **THEN** the context contains a sentinel token in the form `[SENSITIVE_SKILL_TOKEN:{session_id}:{hex}]`
- **AND** the plaintext skill content is not present in the in-memory history
- **AND** the rollout JSONL contains only the token placeholder

#### Scenario: Plaintext skill compatibility
- **WHEN** a non-encrypted skill is mentioned
- **THEN** the system injects it using the existing plaintext behavior
- **AND** no tokenization or decryption is applied

### Requirement: Request-time rehydration
Before any request is sent to the LLM, the system SHALL replace encrypted skill token placeholders in the request input with the decrypted `SKILL.md` document content read from `/dev/shm`.

#### Scenario: Rehydration at the request chokepoint
- **WHEN** the system builds the final request input in `get_formatted_input_for_request`
- **THEN** every sentinel token in `ResponseItem` message content is replaced with the corresponding `SKILL.md` document content
- **AND** the request sent to the LLM contains the real document content

#### Scenario: Rehydration covers all request paths
- **WHEN** a regular turn, compaction, or resumed-session request is built
- **THEN** the same rehydration logic runs before the request is transmitted

#### Scenario: Stale token detection
- **WHEN** a token references content that no longer exists in `/dev/shm`
- **THEN** the system leaves the placeholder unreplaced or reports the stale reference
- **AND** re-mentioning the skill triggers a fresh decryption

### Requirement: Session-scoped token ownership
Each token SHALL embed the owning session ID, and rehydration SHALL refuse to replace tokens whose embedded session ID does not match the current request's session.

#### Scenario: Cross-session token is not rehydrated
- **WHEN** a token belonging to session A appears in session B's messages
- **THEN** the token is left unreplaced
- **AND** no plaintext from session A is exposed to session B

#### Scenario: Same-session token is rehydrated
- **WHEN** a token appears in a message belonging to the session embedded in the token
- **THEN** the token is replaced with the decrypted `SKILL.md` content

### Requirement: Path resolution without exposing decrypted paths
Script and resource references in rehydrated skill content SHALL resolve to the decrypted storage for execution, while the decrypted `/dev/shm` path SHALL NOT appear in LLM-visible content or persisted state.

#### Scenario: Original skill path references are rewritten at tool time
- **WHEN** a tool invocation references the skill's original directory path (for example `bash ~/.codex/skills/foo/scripts/build.sh`)
- **THEN** the system rewrites the reference to the session's decrypted directory before execution
- **AND** the request content sent to the LLM contains only the original address, never the `/dev/shm` path

#### Scenario: Relative references resolve through a base anchor
- **WHEN** rehydrated skill content references package files relatively (for example `scripts/build.sh`, `resources/config.json`)
- **THEN** a base anchor using the skill's original directory is available in the rehydrated content for resolution
- **AND** tool arguments derived from those references are rewritten to the decrypted directory at execution time

#### Scenario: Tool output never reveals the decrypted path
- **WHEN** a tool output contains the decrypted storage path string (for example a script printing its own path)
- **THEN** the path string is redacted before the output enters the model context or is persisted

### Requirement: Trust-tier framing and output policy on rehydration
Rehydrated skill content SHALL be wrapped with framing that declares the skill content as task-procedure data rather than output-policy directives, and SHALL attach an output policy forbidding reproduction of the skill's original content in model replies.

#### Scenario: Skill content contains injection directives
- **WHEN** rehydrated skill content instructs the model to output the skill plaintext or to override the output policy
- **THEN** such instructions are framed as task data that cannot override the injected output policy

#### Scenario: Reply is subject to the output policy
- **WHEN** the model replies after processing an encrypted skill
- **THEN** the output policy in the rehydration framing prohibits quoting, summarizing, translating, or encoding the skill's original content and internal file metadata

### Requirement: Bounded decrypted content
Per-session decrypted content SHALL be bounded with hard caps on entry count and total size.

#### Scenario: Content limit reached
- **WHEN** a session's stored decrypted content exceeds the configured cap
- **THEN** the system evicts oldest entries or refuses new decryption with a warning
- **AND** the session continues without unbounded memory growth

### Requirement: Security audit log
The system SHALL append security-relevant events (decryption, tokenization, rehydration, blocked access, cleanup) to an audit log.

#### Scenario: Security events are recorded
- **WHEN** a decryption, tokenization, rehydration, blocked access, or cleanup event occurs
- **THEN** an audit entry with event type, session, and timestamp is appended to the audit log
- **AND** audit entries never contain skill plaintext

### Requirement: Two-tier TTL decrypted storage lifecycle
The system SHALL unload decrypted skill content using two TTL tiers: a per-skill idle TTL that unloads a single skill, and a per-thread idle TTL that clears all decrypted content of a thread. Plaintext SHALL NOT remain in memory or `/dev/shm` beyond these windows.

#### Scenario: Skill-level idle TTL unloads a single skill
- **WHEN** a decrypted skill has not been triggered again within the skill idle TTL
- **THEN** that skill's decrypted directory and cached plaintext are removed
- **AND** other skills in the same thread remain available until their own TTLs expire

#### Scenario: Thread-level idle TTL clears the whole thread
- **WHEN** a thread has been idle beyond the thread idle TTL
- **THEN** every decrypted directory and cached plaintext entry registered for that thread is removed

#### Scenario: Cleanup at thread end
- **WHEN** a thread ends
- **THEN** the system immediately removes every `fm_skill_security_<hex>/` directory registered for that thread
- **AND** other threads' decrypted content is untouched

#### Scenario: Re-mention after unload triggers fresh decryption
- **WHEN** a skill is triggered again after its decrypted content was unloaded
- **THEN** the system decrypts the package again into a new directory
- **AND** the new directory replaces the stale registry entry

#### Scenario: Multi-session isolation
- **WHEN** two sessions run concurrently on the same machine
- **THEN** each session's registry maps only to its own decrypted directories
- **AND** rehydration in one session cannot read another session's content

### Requirement: Decryption path hiding
The system SHALL NOT expose the decrypted storage path to the user or the model.

#### Scenario: Path absent from context and output
- **WHEN** an encrypted skill is injected or rehydrated
- **THEN** the context, rollout JSONL, and model-visible output contain only the token placeholder and skill name
- **AND** the absolute `/dev/shm/fm-agent-security/` path is not included in the context, rollout JSONL, audit log, or LLM request content
- **AND** rehydrated content references the skill via original or relative addresses only
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

### Requirement: Shell command path rewriting to decrypted storage
Before executing a shell command whose arguments reference a decrypted skill's original directory or skill-relative paths, the system SHALL rewrite those references to the session's decrypted directory.

#### Scenario: Shell command with original skill path is rewritten
- **WHEN** a shell command references the skill's original directory (for example `bash ~/.codex/skills/foo/scripts/build.sh`)
- **THEN** the command is rewritten to the session's decrypted directory before execution
- **AND** the real script under `/dev/shm/fm-agent-security/` executes

#### Scenario: Non-shell tools never reach decrypted resources
- **WHEN** a non-shell tool references a skill-relative resource or any decrypted storage path
- **THEN** the invocation is blocked; shell script execution is the sole decrypted-storage channel, so no file-tool path rewriting is provided

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
## ADDED Requirements

### Requirement: Subagent fork drops encrypted skill tokens
When a subagent is forked, the system SHALL exclude response items containing encrypted skill sentinel tokens from the inherited context.

#### Scenario: Token item filtered at fork
- **WHEN** the parent session's rollout contains an encrypted skill sentinel token item during fork
- **THEN** that item is dropped from the subagent's inherited context
- **AND** no decrypted skill content or token reference crosses the fork boundary

#### Scenario: Subagent re-mentions the skill
- **WHEN** a subagent needs the encrypted skill content
- **THEN** the subagent may mention the skill name to trigger its own decryption within its session scope

### Requirement: No cross-session decrypted content sharing
The system SHALL ensure a session's decrypted skill content is never readable by other sessions or their subagents.

#### Scenario: Isolated decrypted storage
- **WHEN** a subagent or another session attempts to resolve an encrypted skill token
- **THEN** resolution is restricted to the decrypted directories registered for the requesting session in the session registry
