## ADDED Requirements

### Requirement: Sandbox policy carries read-only bind mappings

`FileSystemSandboxPolicy` SHALL carry `readonly_binds: Vec<ReadonlyBind>` where `ReadonlyBind` has a `source` (host path) and `target` (path inside the sandbox). The field SHALL default to an empty list when absent in serialized input, and SHALL round-trip through serde.

#### Scenario: Default policy has no binds
- **WHEN** a policy is deserialized without `readonly_binds`
- **THEN** the policy has an empty `readonly_binds` list

#### Scenario: Binds round-trip through serialization
- **WHEN** a policy with one bind is serialized and deserialized
- **THEN** the bind source and target are preserved

### Requirement: Bubblewrap applies read-only binds to logical skill paths

When building bubblewrap filesystem arguments, each `readonly_bind` SHALL be mounted with `--ro-bind <source> <target>` after writable roots and unreadable masks, SHALL create a missing target directory with `--dir`, and SHALL be skipped when the source does not exist.

#### Scenario: Bind is mounted read-only
- **WHEN** a policy has a bind whose source exists
- **THEN** the bwrap args contain `--ro-bind <source> <target>` after the base mounts

#### Scenario: Missing target is created
- **WHEN** a bind target does not exist in the sandbox view
- **THEN** the bwrap args contain `--dir <target>` before the bind

#### Scenario: Missing source is skipped
- **WHEN** a bind source does not exist
- **THEN** no bind args are emitted for it

#### Scenario: Empty binds leave args unchanged
- **WHEN** a policy has no binds
- **THEN** the bwrap args match the pre-change behavior

### Requirement: Orchestrator injects skill binds only when bwrap applies

`SandboxAttempt` SHALL carry the session's `(decrypted_dir, original_dir)` mappings as `skill_binds`, and `env_for`/`env_for_exec_server` SHALL add them to the permission profile used for sandboxing only when `sandbox_applies_binds` is true (Linux, non-legacy landlock, bwrap actually in effect). Otherwise `skill_binds` SHALL be empty.

#### Scenario: Binds active under bwrap
- **WHEN** the effective sandbox uses bubblewrap and the session is engaged
- **THEN** the transformed profile contains the session's path mappings as readonly binds

#### Scenario: No binds without bwrap
- **WHEN** bubblewrap does not apply (full-access skip, legacy landlock, or non-Linux)
- **THEN** no readonly binds are injected

### Requirement: Guard keeps logical paths when binds apply

When `sandbox_applies_binds` is true, `guard_shell` SHALL keep the command's logical skill path unchanged instead of rewriting it to the decrypted path, and SHALL treat logical skill paths as protected storage (execute-only rules unchanged). When binds do not apply, the existing rewrite-to-decrypted-path behavior SHALL be preserved.

#### Scenario: Logical script execution allowed with binds
- **WHEN** an engaged session executes `bash <logical>/script.sh` and binds apply
- **THEN** the command is allowed unchanged as execute-only

#### Scenario: Logical read blocked with binds
- **WHEN** an engaged session runs `cat <logical>/SKILL.md` and binds apply
- **THEN** the command is blocked

#### Scenario: Legacy rewrite without binds
- **WHEN** binds do not apply
- **THEN** the command is rewritten to the decrypted path exactly as before

### Requirement: Engaged skill script execution is auto-permitted

When the session is engaged and a command is an execute-only skill script execution, the shell approval flow SHALL approve it automatically without prompting the user or routing to guardian review.

#### Scenario: Skill script bypasses approval
- **WHEN** an engaged session runs `bash <logical>/script.sh`
- **THEN** approval is granted without a prompt and without guardian review

#### Scenario: Non-skill command still requires approval
- **WHEN** an engaged session runs an unrelated command
- **THEN** the normal approval flow applies
