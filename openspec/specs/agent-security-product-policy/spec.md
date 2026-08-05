# agent-security-product-policy

## Purpose

产品化边界：engaged 会话强制沙箱（I6）、CLI/app-server 拒绝 full-access 与插件管理（I7）、客户端不暴露配置入口（D10）。

## ADDED Requirements

### Requirement: Engaged execution requires an active sandbox

When the session is engaged and the effective sandbox is not active (`SandboxType::None`), tool execution SHALL be rejected with a clear error.

#### Scenario: Engaged with no sandbox is rejected
- **WHEN** a session is engaged and the selected sandbox type is `None`
- **THEN** tool execution is rejected

#### Scenario: Engaged with sandbox proceeds
- **WHEN** a session is engaged and a sandbox type is active
- **THEN** tool execution proceeds

#### Scenario: Unengaged with no sandbox proceeds
- **WHEN** a session is not engaged and the sandbox type is `None`
- **THEN** tool execution proceeds

### Requirement: CLI rejects full-access execution and plugin management

The `codex` CLI SHALL reject `--sandbox danger-full-access`, `dangerously_bypass_approvals_and_sandbox`, and the `plugin`/`marketplace` subcommands with a product-policy error.

#### Scenario: Full-access flag rejected
- **WHEN** a user passes `--sandbox danger-full-access` or the bypass flag
- **THEN** the command fails with a product-policy message

#### Scenario: Plugin subcommand rejected
- **WHEN** a user runs `codex plugin ...` or `codex plugin marketplace ...`
- **THEN** the command fails with a product-policy message

### Requirement: App-server rejects danger-full-access thread/turn start

`thread/start` and `turn/start` SHALL reject requests whose sandbox policy is danger-full-access.

#### Scenario: Danger thread start rejected
- **WHEN** a client starts a thread with `sandbox_policy = danger-full-access`
- **THEN** the request fails with a product-policy error

#### Scenario: Normal thread start allowed
- **WHEN** a client starts a thread with a normal sandbox policy
- **THEN** the request proceeds as before
