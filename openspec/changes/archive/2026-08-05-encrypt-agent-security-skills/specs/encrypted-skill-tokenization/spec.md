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
