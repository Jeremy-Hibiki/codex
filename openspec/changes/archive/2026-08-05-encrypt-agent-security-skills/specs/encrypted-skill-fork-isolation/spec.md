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
