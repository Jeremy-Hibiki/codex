## ADDED Requirements

### Requirement: Session-level engaged determination

`EncryptedSkillRuntime::is_engaged(session_id)` SHALL return true when the session has at least one registered decrypted skill, and SHALL return false when the session has no registered skills. The determination SHALL be derived from runtime state only and SHALL NOT rely on a separately maintained flag.

#### Scenario: Empty session is not engaged
- **WHEN** a session has no loaded encrypted skills
- **THEN** `is_engaged(session_id)` returns false

#### Scenario: Loaded skill makes session engaged
- **WHEN** a skill has been successfully loaded and registered for the session
- **THEN** `is_engaged(session_id)` returns true

#### Scenario: Clearing the session disengages it
- **WHEN** the session's decrypted state is cleared via `clear_thread`
- **THEN** `is_engaged(session_id)` returns false

### Requirement: In-flight window is covered by engagement

While a decryption for a session is in progress, `is_engaged(session_id)` SHALL return true even if the skill is not yet registered, so that the window between plaintext landing on disk and registry registration is never unprotected. The in-flight marker SHALL be added before plaintext is written and SHALL be removed on both success and failure.

#### Scenario: Engagement during in-flight decryption
- **WHEN** a decryption for the session has started but registration has not completed
- **THEN** `is_engaged(session_id)` returns true

#### Scenario: Failed decryption removes in-flight marker
- **WHEN** decryption fails before registration
- **THEN** the in-flight marker is removed, any partially written decrypted directory is securely wiped, and `is_engaged(session_id)` returns false (unless other skills remain registered)

### Requirement: Path mappings export

`EncryptedSkillRuntime::path_mappings(session_id)` SHALL return the list of `(decrypted_dir, original_dir)` pairs for every skill registered to the session whose `original_dir` is non-empty, and SHALL return an empty list when the session has no such skills.

#### Scenario: Registered skill exposes its mapping
- **WHEN** a skill with a non-empty original directory is registered
- **THEN** `path_mappings` contains the pair `(decrypted_dir, original_dir)` for that skill

#### Scenario: Skill without original directory is excluded
- **WHEN** a skill is registered with an empty original directory
- **THEN** `path_mappings` does not contain an entry for that skill

#### Scenario: No skills yields empty mappings
- **WHEN** the session has no registered skills
- **THEN** `path_mappings` returns an empty list

### Requirement: Engagement does not alter existing guard semantics

This change SHALL only introduce the engagement determination, in-flight tracking, and path-mapping export. It SHALL NOT change the behavior of existing guard or redaction rules for engaged sessions, and SHALL NOT apply any guard or redaction behavior to unengaged sessions (that gating is delivered by a later change).

#### Scenario: Existing guard behavior is untouched
- **WHEN** an engaged session triggers an existing guard rule
- **THEN** the guard outcome matches the pre-change behavior

#### Scenario: Unengaged sessions are not gated by this change
- **WHEN** a session is not engaged
- **THEN** this change introduces no guard or redaction behavior for that session
