# agent-security-final-boundaries

## Purpose

TODO-6/7/8/9 收尾：插件/市场变更 RPC 全禁、settings update 拒绝 danger-full-access、engaged 时配置变更全禁、turn item 输出面明文+路径红act。

## ADDED Requirements

### Requirement: Plugin and marketplace mutation RPCs are disabled

The app-server SHALL reject `marketplace/add`, `marketplace/remove`, `marketplace/upgrade`, `plugin/install`, `plugin/uninstall`, `plugin/share/save`, `plugin/share/updateTargets`, `plugin/share/checkout`, and `plugin/share/delete` with a product-policy error. Read-only listing and read RPCs SHALL remain available.

#### Scenario: Marketplace mutation rejected
- **WHEN** a client calls `marketplace/add` with a source
- **THEN** the request fails with the plugin/marketplace product-policy message

#### Scenario: Plugin install rejected
- **WHEN** a client calls `plugin/install`
- **THEN** the request fails with the plugin/marketplace product-policy message

#### Scenario: Read-only plugin list allowed
- **WHEN** a client calls `plugin/list`
- **THEN** the request is not blocked by the product-policy check

### Requirement: Thread settings update rejects danger-full-access

`thread/settings/update` SHALL reject `sandboxPolicy = danger-full-access` or the danger-full-access permission profile with the same product-policy error as `thread/start` and `turn/start`.

#### Scenario: Settings update danger sandbox rejected
- **WHEN** a client updates thread settings with a danger-full-access sandbox policy
- **THEN** the request fails with the danger-full-access product-policy message

### Requirement: Config mutations are blocked while engaged

When any session is engaged, `config/value/write`, `config/batchWrite`, `experimentalFeature/enablement/set`, `skills/config/write`, and `skills/extraRoots/set` SHALL be rejected. Unengaged processes SHALL retain existing behavior.

#### Scenario: Engaged config write blocked
- **WHEN** a session is engaged and a client writes skills config
- **THEN** the request fails with the config-mutation product-policy message

#### Scenario: Unengaged config write allowed
- **WHEN** no session is engaged
- **THEN** the config write is not blocked by the product-policy check

### Requirement: Turn-item output surfaces redact skill plaintext and paths

`redact_turn_item` SHALL redact known skill plaintext and decrypted storage paths from CommandExecution output fields, FileChange stdout/stderr, WebSearch query, CollabAgentToolCall prompt, DynamicToolCall content/error, and McpToolCall error text before events are emitted or persisted.

#### Scenario: Script output plaintext redacted
- **WHEN** an engaged session runs a skill script that echoes skill plaintext
- **THEN** the rollout and events contain redacted text, not the plaintext

#### Scenario: Decrypted path redacted from command output
- **WHEN** command output contains a decrypted storage path
- **THEN** the persisted/emitted output contains redacted text
