# agent-security-context-gating

## Purpose

定义会话/回合级 AgentSecurityContext，并按 engaged 门控工具 guard 与输出红act；未 engaged 会话保持零行为变化。

## ADDED Requirements

### Requirement: AgentSecurityContext exposes live engagement

`AgentSecurityContext` SHALL carry the session's encrypted-skill runtime and session id, and its `engaged()` SHALL return the live `runtime.is_engaged(session_id)` value rather than a snapshot. A turn without an active encrypted-skill session SHALL carry `None` for its agent-security context.

#### Scenario: Context reflects live state
- **WHEN** a skill is loaded after the context was created
- **THEN** `engaged()` returns true

#### Scenario: No encrypted skills yields None context
- **WHEN** the session has never loaded an encrypted skill
- **THEN** the turn's `agent_security` is `None`

### Requirement: Tool guards are gated on engagement

`before_tool` SHALL return `Allow` for every tool when the session is not engaged, without performing plaintext checks, path checks, or rewrites. When engaged, the pre-change guard behavior SHALL be preserved exactly.

#### Scenario: Unengaged session bypasses all guards
- **WHEN** an unengaged session invokes bash with a command containing the memory root path
- **THEN** the guard returns `Allow`

#### Scenario: Unengaged session bypasses export checks
- **WHEN** an unengaged session invokes a non-shell tool whose arguments contain a memory-root path
- **THEN** the guard returns `Allow`

#### Scenario: Engaged session keeps existing guard behavior
- **WHEN** an engaged session invokes bash with a command reading a decrypted skill directory
- **THEN** the guard outcome matches the pre-change behavior (blocked for non-execution access)

### Requirement: Tool output redaction is gated on engagement

`RedactingToolOutput` SHALL pass tool output through unchanged when the session is not engaged, and SHALL apply the existing unrewrite/redaction behavior when engaged.

#### Scenario: Unengaged output is not redacted
- **WHEN** an unengaged session produces output containing the memory root path
- **THEN** the output reaches the model unchanged

#### Scenario: Engaged output is redacted
- **WHEN** an engaged session produces output containing a decrypted skill path
- **THEN** the output is unrewritten/redacted exactly as before this change

### Requirement: Turn context carries the per-turn context

`TurnContext.agent_security` SHALL be `Some(AgentSecurityContext)` when the session is engaged at turn construction, and `None` otherwise; guard decisions SHALL still consult live runtime state rather than relying solely on the snapshot.

#### Scenario: Engaged turn carries context
- **WHEN** a turn is constructed for an engaged session
- **THEN** `agent_security` is `Some`

#### Scenario: Unengaged turn carries no context
- **WHEN** a turn is constructed for an unengaged session
- **THEN** `agent_security` is `None`
