# Grevo 配置层与权限语义（requirements / config / 项目层）

> 基于 grevo fork 代码核实（工作树 `/workspaces/gpt-web-dev/github/codex/.worktrees/grevo`）。
> 所有结论附代码位置，可用 `grep` 复核。路径以 grevo 命名为准，legacy `codex` 路径仍被兼容。

## 0. 两类层：先分清

| 类别 | 作用 | 合并方式 |
|---|---|---|
| **config layers** | 提供配置**值**（model、permissions、hooks…） | 按优先级 TOML 深合并出 `effective_config` |
| **requirements layers** | 提供管理**约束**（allowed_*、managed profiles、deny_read…） | 单独 compose 成 `ConfigRequirements`，不直接给值，只做校验/兜底/注入 |

关键：requirements 不是 config 层，不会替任何层选值；`[permissions.<name>]` 里定义的 managed profile 会被**注入**到 profiles 目录参与编译（见 §5.2）。

## 1. 文件位置与优先级

### 1.1 requirements 侧（低 → 高）

| 序 | 来源 | 路径 |
|---|---|---|
| 1 | system requirements | `/etc/grevo/requirements.toml`（legacy `/etc/codex/requirements.toml`） |
| 2 | cloud bundle requirements | 企业托管云配置包 |
| 3 | legacy managed_config.toml 重解释 | `/etc/grevo/managed_config.toml`（legacy `/etc/codex/managed_config.toml`） |
| 4 | macOS managed preferences (MDM) | 仅 macOS |

出处：`config/src/loader/mod.rs:91-96`、`:167-200`、`layer_io.rs:19-22`。

### 1.2 config 侧（低 → 高）

| 序 | 来源 | 路径 / 触发 |
|---|---|---|
| 1 | system config | `/etc/grevo/config.toml`（legacy `/etc/codex/config.toml`，`mod.rs:55-57,669-690`） |
| 2 | cloud bundle config | 企业托管云配置包 |
| 3 | user config | `${GREVO_HOME}/config.toml`，默认 `~/.grevo/config.toml`；`GREVO_HOME` 优先、`CODEX_HOME` 兼容（`utils/home-dir/src/lib.rs:11-22,69-78`） |
| 4 | profile | `${GREVO_HOME}/<name>.config.toml`（`--profile` 选中时） |
| 5 | **project layers** | 从 cwd 沿祖先链到 **project root**（含两端），每层读 `<dir>/.grevo/config.toml`，无 `.grevo/` 则读 legacy `<dir>/.codex/config.toml`（`mod.rs:1266-1274` scan、`:988-998`、`:1306`） |
| 6 | runtime / CLI | `--config`、`--add-dir` 等（SessionFlags 层） |
| 7 | thread config | 线程级配置层（`insert_layer_by_precedence`） |
| 8 | legacy managed_config.toml | **整份压顶**，作为"best effort"最高层（`mod.rs:380-402`），处于逐步淘汰 |

出处：`config/src/loader/README.md:34-44`、`mod.rs:240-378`。

**项目级配置文件位置**：`<dir>/.grevo/config.toml`（该目录没有 `.grevo/` 时读 legacy `<dir>/.codex/config.toml`）。
裸 `<project>/config.toml` 不参与加载。

**project root 的判定**：`project_root_markers`（config 顶层数组，默认 `[".git"]`）从 cwd 向上找**最近**一个带 marker 的目录（`mod.rs:1198-1224`）；设为空数组则 project root = cwd。项目层扫描止于 project root，不到文件系统根。

### 1.3 `managed_config.toml` 的双重身份

同一份 `/etc/grevo/managed_config.toml` 被用两次（`mod.rs:167-200,380-402`）：

1. **作为 requirements 层**（序 3）：只映射两个字段——
   `approval_policy` → `allowed_approval_policies = [<值>]`；
   `approvals_reviewer` → `allowed_approvals_reviewers`（`auto_review` 会补 `user`）（`mod.rs:798-851`）。
2. **作为 config 层**（序 8，最高）：整份 TOML 直接压到最顶，字段按普通 config 合并。

## 2. config 层合并规则

`merge_toml_values`（`config/src/merge.rs:57-86`）：

- **表**：递归合并，高层优先；
- **标量、数组**：高层**整体替换**，不存在跨层追加；
- 特例：`shell_environment_policy` 的数组/键值两种表示互斥替换；`[permissions.*.network.domains]` 键名大小写规范化。

直接推论：

- ⚠️ 数组型字段（如 `project_root_markers`、profile 内的列表值）跨层是**替换**：高层一旦设了，低层的同名数组被整体覆盖；
- `mcp_servers`、列表型字段同理；
- 表型字段（`[permissions.<profile>.filesystem]`、`[permissions.<profile>.workspace_roots]`、`[projects]`）跨层按键并集，同键高层覆盖。

**⚠️ disabled 层不参与**：项目层未通过 trust 时带 `disabled_reason`，`effective_config()` 整层剔除（`config/src/state.rs:541`、`loader/README.md:42-44`）。界面上仍可见，但不生效。

**项目层 denylist（与 trust 无关，永远剥除）**（`mod.rs:70-82`）：
`openai_base_url`、`chatgpt_base_url`、`apps_mcp_product_sku`、`model_provider(s)`、`notify`、`profile(s)`、
`experimental_realtime_webrtc_call_base_url`、`experimental_realtime_ws_base_url`、`otel`。
理由：项目内容不可决定凭据去向和本地命令（`sanitize_project_config`，`mod.rs:1004-1013`）。

**strict 校验仅对 trusted 项目层启用**（`mod.rs:1331-1340`）：untrusted 层的解析错误会被吞掉，只留下一个空层。

## 3. requirements 组合规则

`compose_requirements`（`config/src/requirements_layers/stack.rs:17-30`）：

- 与 config 层同序，低 → 高；
- 多数字段按 TOML 合并（高层覆盖标量/数组，递归扩展表）；
- 特例：
  - `permissions.filesystem.deny_read`：跨层**并集**（高优先级在前）；
  - `hooks`：高优先级事件组在前，managed-dir 冲突 fail-closed；
  - `rules.prefix_rules`：高优先级规则在前；
  - `remote_sandbox_config`：每层先按 `hostname_patterns` 求值再合并（不匹配本机 host 的层不贡献字段）；
- ⚠️ `default_permissions` 依赖 `allowed_permission_profiles`：设了前者没后者 → 启动报错（`core/src/config/mod.rs:4697-4717`）。

## 4. trust：项目层生效的前提

- trust 数据源：`[projects]` 表（`config.toml` 顶层），key 为项目路径（精确/规范化匹配，支持 `~` 展开，**无 glob**），值 `{ trust_level = "trusted" | "untrusted" }`（`config_toml.rs:694-706`）。
- trust 上下文在项目层加载**之前**计算，只含 system + cloud + user 层（`mod.rs:315-330,1037-1086`）。**requirements.toml 没有 `[projects]`，写进去无效。**
- 匹配顺序：目录自身 key → project_root key → git repo root key（`mod.rs:888-930`）。所以信任共同 git 根可覆盖整树。
- ⚠️ 未 trust（也无 untrusted 决定）→ 项目层 disabled，全部配置都不生效（UI 里能看到 disabled 原因）。
- 旁路：app-server `thread/start` 带 cwd 且解析为 workspace-write/full-access 时，会**自动把该项目写入 user config.toml 的 trusted**（`app-server/README.md:142`）。

## 5. permission profile 语义：`default_permissions` / `filesystem` / `workspace_roots`

### 5.1 profile 的选择

`default_permissions` 决定用哪个 profile，取值来源按优先级（`resolve_default_permissions`，`core/src/config/mod.rs:4664-4694`）：

1. runtime override（CLI/UI/app-server 请求参数）；
2. config 层（含项目层）的 `default_permissions`；
3. requirements 的 `default_permissions`（仅当设了 `allowed_permission_profiles` 时作 fallback）。

**选择校验**：存在 `allowed_permission_profiles` 时，被选 profile 必须在表中且值为 true——**精确匹配，无通配，缺省 deny**（`mod.rs:4760-4766`）；不允许则告警并回落 requirements 的 default。

**⚠️ 任何一层出现 `sandbox_mode`（含 CLI `--sandbox`）都会让整条链退出 profiles 模式。** 只用 profile 的环境里，这是配置静默失灵的常见原因。

### 5.2 profile 编译

编译规则（`config/src/permissions_toml.rs:40-104`、`core/src/config/permissions.rs:339-441`）：

- `extends` 链：parent-first 合并，子覆盖同键；⚠️ 循环 → 报错；父不存在 → 报错；
- 可扩展的 builtin 父只有 `:read-only`、`:workspace`；`:danger-full-access` 不可 extends；
- 自定义 profile 名不得以 `:` 开头（builtin 保留）；
- ⚠️ **managed（requirements 定义）profile 与 config 层自定义 profile 同名 → 启动硬报错**（`mod.rs:4616-4626`）。项目层不能往 managed profile 名底下加键；
- `network.enabled`：true→放开网络，false/缺省→Restricted。

**managed profile 注入**：requirements 的 `[permissions.<name>]`（不含保留键 `filesystem`）合入 profiles 目录（`merge_managed_permission_profiles`，`mod.rs:4606-4633`），`extends` 解析发生在合并之后，因此 config 自定义 profile 可以 `extends = "<managed 名>"`。

### 5.3 `filesystem` 表（profile 内）

DSL（`core/src/config/permissions.rs:531-700,796-870`）：

- key 形态：绝对路径、`~/...`、⚠️ glob（仅 deny 可用任意 glob；read/write 仅允许精确路径或结尾 `/**`，否则告警且该平台不生效）、`:special`；
- `:special` 占位符：`:root`、`:minimal`、`:workspace_roots`、`:tmpdir`、`:slash_tmp`；未知 `:` 值前向兼容（告警忽略，不报错）；
- 值形态：字符串（`"read" | "write" | "deny"`）或**子表（scoped）**——`[permissions.<p>.filesystem.":workspace_roots"] "<subpath>" = "write"`；
- ⚠️ scoped 子路径必须是**纯子孙路径**，含 `.`/`..` 直接报错（`parse_relative_subpath`，`permissions.rs:864-881`）。**无法表达"workspace 的兄弟目录"**，跨项目可写只能写绝对路径键；
- 表内键跨层并集合并（见 §2），同键高层覆盖。

### 5.4 `workspace_roots` 表（profile 级）

```toml
[permissions.<name>.workspace_roots]
"/abs/or/~/path" = true   # false 可显式禁用某 root
```

- 编译：`compile_permission_profile_workspace_roots`（`permissions.rs:430-466`），相对路径按 **policy cwd** 解析，收集 enabled 的；
- 生效方式：作为"物化 project roots"写进 `FileSystemSandboxPolicy`（`with_materialized_project_roots_for_workspace_roots`，`protocol/src/permissions.rs:867-881`）。这些 root 获得与 workspace root 等同的可写地位；profile 里的 `:workspace_roots` 符号条目也会为这些 root 物化出具体条目；
- 与 `filesystem` 条目的关系：`filesystem` 是细粒度 allow/deny 规则；`workspace_roots` 是"整目录提升为 workspace 根"。二者作用于同一张 policy 表，deny 规则仍可对 workspace_roots 内的子路径生效（policy 条目按序匹配）。

### 5.5 其他注意事项

- `runtimeWorkspaceRoots`（app-server 实验字段）：**替换**语义（须显式含 cwd），不是追加；与 config 的 roots 是另一条注入路径。
- requirements `[permissions.filesystem] deny_read`：全局禁读（支持 glob），作用于 catalog allowed 计算与运行时策略；这是 requirements 里唯一的"黑名单形状"字段（`config_requirements.rs:552-618`）。
- `approval_policy` 不是 requirements 字段（requirements 里是 `allowed_approval_policies` 列表），写错键会被直接忽略。
- `thread/start`/`resume`/`fork` 可带 `permissions` 覆盖，优先级最高（§5.1 序 1）。

## 6. app-server 如何参与：何时读配置、何时发配置

### 6.1 读（构建 Config / ConfigLayerStack 的时机）

| 时机 | 行为 | 代码 |
|---|---|---|
| 服务器启动 | 读一次配置做 bootstrap（auth、thread config loader 发现、feature enablement） | `app-server/src/lib.rs:505-508` |
| `thread/start` / `resume` / `fork` | **按请求 cwd 从磁盘重建完整层栈**（含该项目层 + trust 判定），线程持有自己的 config snapshot | `config_manager.rs:164-178`、core `ConfigBuilder` |
| `turn/start` | 在线程 snapshot 上叠加 per-turn overrides（`cwd`、`runtimeWorkspaceRoots`、`permissions`） | `thread_processor.rs:3094-3155` |
| `config/read`、`configRequirements/read`、`permissionProfile/list` | 请求到达时即时从磁盘读。`config/read` 可带 `cwd`（决定含不含项目层）和 `includeLayers`（返回每层内容 + per-key origins） | `config_processor.rs:85-124`、`config_manager_service.rs:458-460` |
| `config/value/write` | 写 user config.toml，发 plugin toggle 事件；**不自动重载** | `config_processor.rs:127-135` |
| `config/batchWrite` | 写 user config.toml；非 session-defaults 的编辑会清 plugin/skills 缓存；`reloadUserConfig: true` 时热更新已加载线程（保留会话层；model、reasoning-effort、service-tier、personality 等 session-static 项不重载） | `config_processor.rs:136-157,294-300`、`config_manager.rs:159-178` |
| `experimentalFeature/enablement/set` | 更新运行时开关后触发 reload | `config_processor.rs:258-289` |
| `thread/start` 副作用 | 解析结果为 workspace-write/full-access 时，自动把该项目写进 user config.toml 的 trusted | `app-server/README.md:142` |

没有文件 watch：磁盘上的 `.grevo/config.toml` 改动，已加载的线程不会自动感知；要么客户端写配置时带 `reloadUserConfig`，要么等下一次 `thread/start`/`resume` 重建。

**⚠️ `thread/start`/`resume`/`fork` 的 `sandbox` 参数等价于 `sandbox_mode` override**：一旦传入，该线程整条链退出 profiles 模式，`default_permissions` / `[permissions]` 全部跳过（协议注释 "Cannot be combined with `sandbox`"，`v2/thread.rs:89`）。要按线程指定权限，传 `permissions: "<profile id>"`；要加可写目录，传 `runtimeWorkspaceRoots`（替换语义，须含 cwd）。

**⚠️ `turn/start` 的显式 `sandboxPolicy` 参数会整体替换配置推导的策略**（`command_exec_processor.rs:238-269`）：`from_legacy_sandbox_policy_for_cwd` 只从请求参数构造 `FileSystemSandboxPolicy`，配置里的 `[sandbox_workspace_write].writable_roots` / `network_access` 不参与，只过 requirements 校验。已知客户端行为：VS Code Agent Host 每个 turn 都显式发送自己构造的 `sandboxPolicy`（writableRoots = 工作区目录 + VS Code 设置 `codex.additionalDirectories`，见 vscode `agentHost/node/codex/codexAgent.ts:1569-1621`），因此该客户端下 codex 配置文件里的 writable_roots 不生效，加目录要用 `codex.additionalDirectories` 设置或让客户端改用 `permissions` 字段。

### 6.2 发（server → client）

| 通道 | 内容 |
|---|---|
| `thread/started` 及 start/resume/fork 响应 | `runtimeWorkspaceRoots`、`activePermissionProfile`（隐式 builtin 时为 null）、legacy `sandbox` 投影 |
| `permissionProfile/list` 响应 | profile catalog：每个 id + `allowed`（requirements allow map ∩ 编译是否通过） |
| `config/read` 响应 | 合并后的配置值；`includeLayers: true` 时附每层来源与 per-key origins |
| `configRequirements/read` 响应 | 当前生效的 requirements 约束（allow-list、managed profiles、deny_read 等） |

没有主动推送配置变更的 notification：客户端用 `config/read`、`permissionProfile/list` 拉取；线程的权限状态随 start/resume/fork 响应一次性给出。

## 7. 冲突速查

| 场景 | 结果 |
|---|---|
| ⚠️ 项目层定义与 requirements 同名 profile | 启动**报错** |
| 两层都设 `[permissions.foo.filesystem]` 的不同键 | 并集；同键高层覆盖 |
| 项目层 `extends` managed profile | 允许（合并后解析） |
| ⚠️ 项目 untrusted | 整层禁用，配置不生效（UI 可见） |
| ⚠️ 任何一层写 `sandbox_mode` | 整条链退出 profiles 模式 |
| ⚠️ requirements `default_permissions` 无 allow map | 启动报错 |
| allow map 之外的 profile 名 | 不可选（缺省 deny，无通配） |
| ⚠️ scoped 子路径写 `../x` | 报错（仅纯子孙路径） |
| ⚠️ read/write glob（非 `/**` 结尾） | 告警，非 macOS 不生效；deny glob 任意 |
| 兄弟项目各有一份 `.grevo/config.toml` | 同会话只加载 cwd 链（cwd→project root）上的层，兄弟配置互不合并 |

## 8. 验证手段

- 生效层与来源：`configManager` 系 RPC / TUI 的 config layers 视图（disabled 层会标注原因与来源文件）。
- origins：每个合并键标注 `ConfigLayerSource`（System / User / Project / SessionFlags / LegacyManagedConfigTomlFromFile…）。
- 沙箱实测：在目标目录实际写入一次；workspace-write 走 Landlock/Seatbelt，deny 规则即时可见。
- `GREVO_HOME` 指向干净目录 + `--config` 最小复现，逐层加回定位冲突。
