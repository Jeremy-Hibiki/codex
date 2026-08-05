## ADDED Requirements

### Requirement: Process-level engagement registry

Every constructed `EncryptedSkillRuntime` SHALL register itself in a process-level registry (holding a weak reference), and `any_engaged()` SHALL return true when any registered runtime reports `is_engaged` for any session. `engaged_guarded_paths()` SHALL return the mem root plus the decrypted directories of all engaged sessions across registered runtimes.

#### Scenario: No runtimes engaged
- **WHEN** no registered runtime is engaged
- **THEN** `any_engaged()` returns false and `engaged_guarded_paths()` is empty

#### Scenario: One engaged runtime
- **WHEN** one registered runtime has a loaded skill
- **THEN** `any_engaged()` returns true and `engaged_guarded_paths()` contains that session's decrypted directory

#### Scenario: Dropped runtime is ignored
- **WHEN** a runtime is dropped
- **THEN** its engagement no longer contributes to `any_engaged()` or `engaged_guarded_paths()`

### Requirement: Telemetry tool preview is redacted before logging

When a tool result is produced, the `log_preview` handed to telemetry SHALL be the engaged-redacted preview (unrewrite + path/plaintext redaction), not the raw output.

#### Scenario: Preview redacted when engaged
- **WHEN** an engaged session produces output containing a decrypted skill path
- **THEN** the telemetry preview does not contain the decrypted path

#### Scenario: Preview unchanged when unengaged
- **WHEN** an unengaged session produces output
- **THEN** the telemetry preview matches the raw preview

### Requirement: RPC filesystem guard

When `any_engaged()` is true, app-server filesystem RPCs SHALL block access to guarded paths: `fs/readFile`, `fs/readDirectory`, `fs/getMetadata`, `fs/watch` SHALL block paths under a guarded path; `fs/writeFile`, `fs/createDirectory`, `fs/copy`, `fs/remove` SHALL block targets under a guarded path.

#### Scenario: Engaged process blocks guarded fs read
- **WHEN** a client calls `fs/readFile` with a path under a guarded path and the process is engaged
- **THEN** the request is blocked with a generic error

#### Scenario: Unengaged process allows fs read
- **WHEN** no session is engaged
- **THEN** `fs/readFile` behaves as before

### Requirement: RPC execution guard

When `any_engaged()` is true, `thread/shellCommand` and `process/spawn` SHALL be blocked outright, and `command/exec` SHALL be blocked when its command references a guarded path.

#### Scenario: Engaged process blocks unsandboxed execution
- **WHEN** a client calls `thread/shellCommand` or `process/spawn` while engaged
- **THEN** the request is blocked

#### Scenario: Engaged process blocks command referencing guarded path
- **WHEN** a client calls `command/exec` with a command containing a guarded path while engaged
- **THEN** the request is blocked

### Requirement: RPC injection and metadata guard

When `any_engaged()` is true, `thread/inject_items`, `thread/name/set`, `thread/goal/*`, and `thread/metadata/update` SHALL block arguments that reference a guarded path.

#### Scenario: Injection referencing guarded path blocked
- **WHEN** a client injects items containing a guarded path while engaged
- **THEN** the request is blocked

#### Scenario: Metadata referencing guarded path blocked
- **WHEN** a client sets a thread name/goal containing a guarded path while engaged
- **THEN** the request is blocked
