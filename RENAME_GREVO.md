# Grevo 改名清单：Codex → Grevo 全量重命名盘点

> 目标：基于 Codex 二次开发的产品启用新商标 **Grevo**。本文档逐一梳理仓库中每一个出现 "Codex" 的位置，给出改名映射、影响面与兼容性红线。
> 范围约定（用户明确要求）：① 可执行文件 `codex` → `grevo`；② 约定目录 `.codex` → `.grevo`（含插件、配置等所有衍生约定）。
> 标注说明：✅ = 已人工逐行复核确认；其余条目来自逐 crate 扫描并带 file:line 溯源，实施前建议再跑一次全文 grep 验证。
> **已确认的关键前提（2026-09-04）**：产品与 OpenAI 无关，不会接入 OpenAI/ChatGPT 官方服务。因此所有面向 OpenAI 后端的协议项（URL、`x-codex-*`/`x-openai-*` 头、OAuth、ChatGPT 登录、OpenAI 模型目录）从"保留"改为"随自建后端重命名，或整块移除"。

---

## 0. 结论速览

- 规模：Rust 工作区 **129 个成员 crate + 6 个隐式 path 依赖 crate**，其中约 **110 个包名以 `codex-` 开头**；共 **31 个二进制目标**；`codex-rs` 源码中 "codex"（不分大小写）字面量出现约 **5.4 万次**。
- 改名不是一刀切：本清单把所有出现点分成三类——
  - **A 直接改/移除**：crate/包名、二进制名、用户可见文案、构建脚本、发行资产名；以及原属 OpenAI 契约的 URL/头/OAuth/ChatGPT 登录（既然不接官方服务，这些改为随自建后端重命名，或整块移除）。
  - **B 改但要带兼容/迁移**：`~/.codex` 主目录、`CODEX_HOME`、仓库级 `.codex/`、`.codex-plugin/`、`originator`、MCP serverInfo/tool 名、`clientInfo`、`x-codex-*` 请求头（语义保留、名字随自建后端同批切换）等（有落盘状态或对端识别）。
  - **C 保留**：仅剩法律项（Apache-2.0/LICENSE/NOTICE 中 OpenAI 版权归属，依法保留）。`CODEX_SANDBOX*` 由"禁改"降级为"可选改"——它是纯内部父子进程契约，与 OpenAI 无关，改名只需原子替换并同步 AGENTS.md 条款（见 §10.2）。
- 最大风险点：`~/.codex` 用户数据迁移、`x-codex-*` 头/协议与自建后端的同批切换、ChatGPT 登录链路的替换或移除、npm/PyPI 新发布身份。
- **只想最小化改名**（可执行文件 + 目录/配置约定 + 用户可见品牌，其余全保留）：直接看 §15，P0 约 30 个文件、<400 行 diff。

---

## 1. 规模概览（扫描基线）

| 指标                                                             | 数值                                                                                      | 备注                                                                      |
|------------------------------------------------------------------|-------------------------------------------------------------------------------------------|---------------------------------------------------------------------------|
| workspace 成员                                                   | 129                                                                                       | `codex-rs/Cargo.toml:2-132`                                               |
| 隐式 path 依赖 crate                                             | 6                                                                                         | `chatgpt/`、`message-history/`、`windows-sandbox-rs/`、3 个测试支持 crate |
| `codex-*` 包名                                                   | ~110                                                                                      | 全部在 `[workspace.dependencies]`（`Cargo.toml:156-278`）有别名           |
| 非 codex 前缀的 fork 自有 crate                                  | 3                                                                                         | `fm/license`、`fm/encrypted-skills`、`fm/product-policy`                  |
| 二进制目标                                                       | 31                                                                                        | 见 §2                                                                     |
| "codex" 字面量（codex-rs，rs/toml/md/json/bzl，不含 Cargo.lock） | ≈54,000 次                                                                                | 粗略计数                                                                  |
| 依赖 codex 的非 Rust 目录                                        | sdk(97 文件)、.github(62)、scripts(32)、codex-cli、docs、.devcontainer、release、bazel 等 |                                                                           |

自动生成、改名后需重生成（不要手改）：`codex-rs/Cargo.lock`、`MODULE.bazel.lock`、`pnpm-lock.yaml`、`.devcontainer/codex-install/pnpm-lock.yaml`、`scripts/uv.lock`、app-server-protocol / hooks 的 schema fixtures、`core/config.schema.json`、TUI insta 快照（约 38 个快照文件含 "Codex" 文案，集中在 `codex-rs/tui/src/bottom_pane/snapshots/`）。

---

## 2. 可执行文件 / 二进制名（需求 ①：`grevo`）

### 2.1 核心映射

| 现名                                                                                                    | 新名                           | 定义位置                                                     |
|---------------------------------------------------------------------------------------------------------|--------------------------------|--------------------------------------------------------------|
| `codex`                                                                                                 | **`grevo`**                    | `codex-rs/cli/Cargo.toml:8-10`（`[[bin]] name = "codex"`）✅  |
| `codex-tui`                                                                                             | `grevo-tui`                    | `codex-rs/tui/Cargo.toml:8-10`                               |
| `codex-exec`                                                                                            | `grevo-exec`                   | `codex-rs/exec/Cargo.toml:8-10`                              |
| `codex-mcp-server`                                                                                      | `grevo-mcp-server`             | `codex-rs/mcp-server/Cargo.toml:7-9`                         |
| `codex-app-server`                                                                                      | `grevo-app-server`             | `codex-rs/app-server/Cargo.toml:7-9`                         |
| `codex-linux-sandbox`                                                                                   | `grevo-linux-sandbox`          | `codex-rs/linux-sandbox/Cargo.toml:7-9`                      |
| `codex-execve-wrapper`                                                                                  | `grevo-execve-wrapper`         | `codex-rs/shell-escalation/Cargo.toml:7-9`                   |
| `codex-execpolicy`                                                                                      | `grevo-execpolicy`             | `codex-rs/execpolicy/Cargo.toml:13-15`                       |
| `codex-file-search`                                                                                     | `grevo-file-search`            | `codex-rs/file-search/Cargo.toml:7-9`                        |
| `codex-code-mode-host`                                                                                  | `grevo-code-mode-host`         | `codex-rs/code-mode-host/Cargo.toml:7-9`                     |
| `codex-responses-api-proxy`                                                                             | `grevo-responses-api-proxy`    | `codex-rs/responses-api-proxy/Cargo.toml:12-14`              |
| `codex-stdio-to-uds`                                                                                    | `grevo-stdio-to-uds`           | `codex-rs/stdio-to-uds/Cargo.toml:7-9`                       |
| `codex-write-config-schema`                                                                             | `grevo-write-config-schema`    | `codex-rs/core/Cargo.toml:11-13`                             |
| `codex-command-runner`                                                                                  | `grevo-command-runner`         | `codex-rs/windows-sandbox-rs/Cargo.toml:17-19`（仅 Windows） |
| `codex-windows-sandbox-setup`                                                                           | `grevo-windows-sandbox-setup`  | `codex-rs/windows-sandbox-rs/Cargo.toml:13-15`（仅 Windows） |
| `codex-app-server-test-client` / `codex-app-server-test-notify-capture` / `codex-thread-manager-sample` | `grevo-*`（测试/样例，可选改） | 各自 Cargo.toml                                              |
| `apply_patch`、`applypatch`、`bwrap`、`md-events`、`exec-server` 等                                     | 无 codex 前缀，不改            | —                                                            |

### 2.2 argv[0] 多重人格分发（`codex-arg0` crate）——改名必须同步的隐式契约

`codex-rs/arg0/src/lib.rs` 实现单个可执行文件按 argv[0] / argv[1] 哨兵切换行为，以下硬编码字符串必须与二进制名**同批原子改名**：

- `arg0/src/lib.rs:20-23`：`APPLY_PATCH_ARG0 = "apply_patch"`、`"applypatch"`、`EXECVE_WRAPPER_ARG0 = "codex-execve-wrapper"`。
- `codex-rs/sandboxing/src/landlock.rs:7`：`CODEX_LINUX_SANDBOX_ARG0 = "codex-linux-sandbox"`（arg0 在 `arg0/src/lib.rs:95` 据此分发）。
- argv[1] 哨兵参数：`"--codex-run-as-apply-patch"`（`codex-rs/apply-patch/src/lib.rs:41`）、`"--codex-run-as-fs-helper"`（`codex-rs/exec-server/src/fs_helper.rs:45`）、`"--codex-run-as-arg0-exec-helper"`（`codex-rs/exec-server/src/arg0_exec_helper.rs:4`）。
- `arg0/src/lib.rs:292`：`ILLEGAL_ENV_VAR_PREFIX = "CODE_"`→`"GREVO_"`（禁止传入子进程的环境变量前缀）。
- `arg0/src/lib.rs:294-300`：启动时加载 `~/.codex/.env`（跟随 §3 主目录改名）。
- `arg0/src/lib.rs:229,353,369,382-389`：线程名 `"codex-main"`、临时目录 `codex_home/tmp/arg0`、临时前缀 `"codex-arg0"`、PATH 符号链接名（`apply_patch`/`codex-linux-sandbox`/`codex-execve-wrapper`）。

### 2.3 二进制内嵌的名称字符串

- `codex-rs/cli/src/main.rs:107-108`：`bin_name = "codex"`、`override_usage = "codex [OPTIONS] ..."` ✅
- `codex-rs/cli/src/main.rs:2664`：shell 补全生成硬编码 `let name = "codex";` ✅
- `codex-rs/cli/src/main.rs:1116`：`ExecCli::try_parse_from(["codex", "exec"])`。
- `codex-rs/execpolicy/src/main.rs:9,11`、`stdio-to-uds/src/main.rs:17`、`core/src/bin/config_schema.rs:10`：clap `name`。
- `codex-rs/tui/src/lib.rs:231`：日志文件名 `"codex-tui.log"` ✅；`:405,569` `client_name: "codex-tui"` ✅。
- `codex-rs/install-context/src/lib.rs:9-12`：安装布局常量 `"codex-package.json"`、`"codex-path"`、`"codex-resources"` ✅。

### 2.4 沙箱/子进程边界上的二进制名

- `codex-linux-sandbox`：arg0 常量（上）、`/tmp` 代理 socket 目录前缀 `"codex-linux-sandbox-proxy-"`（`codex-rs/linux-sandbox/src/proxy_routing.rs:49`，pid 解析有安全语义）、错误文案（`protocol/src/error.rs:152`）、doctor 标签（`cli/src/doctor.rs:1679` 等）。
- Windows：`SETUP_EXE_FILENAME = "codex-windows-sandbox-setup.exe"`（`windows-sandbox-rs/src/setup.rs:54`）、**Windows 服务名** `WFP_SETUP_SERVICE_NAME = "codex-windows-sandbox-setup"`（`windows-sandbox-rs/src/wfp_setup.rs:11`，改名需处理旧服务卸载）、helper `codex-command-runner.exe`（`helper_materialization.rs:30` 等）。
- `codex-execve-wrapper` 的 `EXEC_WRAPPER` + `CODEX_ESCALATE_SOCKET` 协议（见 §4）。

---

## 3. 目录与文件约定（需求 ②：`.codex` → `.grevo`）

### 3.1 用户主目录 `~/.codex` → `~/.grevo`

- 唯一权威解析点：`codex-rs/utils/home-dir/src/lib.rs:13-60` `find_codex_home()`——先读环境变量 `CODEX_HOME`，未设置则 `home_dir().push(".codex")`（:59）✅。三平台一致：Windows 下即 `%USERPROFILE%\.codex`（无 AppData/注册表特判）。
- 独立硬编码 `~/.codex` 的重复点（容易漏改）：`codex-rs/state/src/bin/logs_client.rs:150-154`、`scripts/install/install.sh:18`、`scripts/install/install.ps1:902`、`sdk/python/src/openai_codex/client.py:864`。
- **配套环境变量：`CODEX_HOME` → `GREVO_HOME`**（约 70 处引用；另有直接 `std::env::var("CODEX_HOME")` 的点：`windows-sandbox-rs/src/bin/setup_main/win.rs:394`、`linux-sandbox/src/proxy_routing.rs:330`）。
- 迁移建议：新版启动时若 `~/.grevo` 不存在而 `~/.codex` 存在 → 整体迁移（或软链/双读过渡一个版本）。

`$CODEX_HOME` 下的全部落盘路径（改名时随主目录自动变化，无需逐个改；列出以备数据迁移与文档更新）：

| 路径                                                                                                                                                                                                   | 用途                      | 定义位置                                                                                                                                                                                                                                                                                    |
|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|---------------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `config.toml`、`*.config.toml`                                                                                                                                                                         | 配置                      | `core/src/config/mod.rs:273-274`                                                                                                                                                                                                                                                            |
| `auth.json`                                                                                                                                                                                            | 登录凭证                  | `login/src/auth/storage.rs:151`                                                                                                                                                                                                                                                             |
| `.env`                                                                                                                                                                                                 | 启动时加载                | `arg0/src/lib.rs:298-300`                                                                                                                                                                                                                                                                   |
| `AGENTS.md` / `AGENTS.override.md`                                                                                                                                                                     | 全局指令                  | `codex-home/src/instructions/mod.rs:9-10`                                                                                                                                                                                                                                                   |
| `sessions/`、`archived_sessions/`（`rollout-*.jsonl`）                                                                                                                                                 | 会话回放                  | `rollout/src/lib.rs:25-26`、`rollout/src/recorder.rs:1553-1570`                                                                                                                                                                                                                             |
| `log/`、`log/codex-tui.log`                                                                                                                                                                            | 日志                      | `core/src/config/mod.rs:3995`、`tui/src/lib.rs:231` ✅                                                                                                                                                                                                                                       |
| `history.jsonl`                                                                                                                                                                                        | 历史                      | `message-history/src/lib.rs:52`                                                                                                                                                                                                                                                             |
| `version.json`                                                                                                                                                                                         | 更新检查缓存              | `tui/src/updates_cache.rs:18-21`                                                                                                                                                                                                                                                            |
| `skills/`（含 `.system/` 与 **`.codex-system-skills.marker`**）                                                                                                                                        | 技能                      | `skills/src/lib.rs:22-27` ✅（marker 文件名本身含 codex，需改为 `.grevo-system-skills.marker`，注意旧标记清理）                                                                                                                                                                              |
| `agents/`                                                                                                                                                                                              | agent 角色/迁移子代理     | `core/src/config/agent_roles.rs:77`、`external-agent-migration/src/service.rs:590`                                                                                                                                                                                                          |
| `memories/`、`hooks.json`、`rules/default.rules`                                                                                                                                                       | 记忆/钩子/execpolicy 规则 | `ext/memories/src/local.rs:31`、`external-agent-migration/src/detect/mod.rs:165`、`core/src/exec_policy.rs:50-52,842`                                                                                                                                                                       |
| `ipc/ipc.sock`、`app-server-control/`、`app-server-daemon/`                                                                                                                                            | IPC 与常驻服务            | `tui/src/ide_context/ipc.rs:180`、`app-server-transport/src/transport/mod.rs:52-70`、`app-server-daemon/src/lib.rs:265-275`                                                                                                                                                                 |
| `packages/standalone/releases                                                                                                                                                                          | current`                  | 自更新安装布局                                                                                                                                                                                                                                                                              | `install-context/src/lib.rs:241-253`、`app-server-daemon/src/managed_install.rs:19-25` |
| `*.sqlite`（5 个库，详见 §3.4）                                                                                                                                                                        | 状态库                    | `state/src/sqlite.rs:24-28`                                                                                                                                                                                                                                                                 |
| `plugins/cache/...`、`plugins/.marketplace-plugin-source-staging`、`.tmp/marketplaces`、`cache/remote_plugin_catalog`                                                                                  | 插件                      | `core-plugins/src/loader.rs:1488`、`npm_source.rs:11`、`installed_marketplaces.rs:12`、`remote.rs:902`                                                                                                                                                                                      |
| 其余：`themes/`、`visualizations/`、`generated_images/`、`models_cache.json`、`environments.toml`、`managed_config.toml`/`requirements.toml`、`cloud-config-bundle-cache.json`、`.sandbox*`（Windows） | —                         | `tui/src/theme_picker.rs:284`、`inline_visualization.rs:66`、`ext/image-generation/src/artifact.rs:5`、`models-manager/src/manager.rs:26`、`exec-server/src/environment.rs:177-192`、`config/src/state.rs:67-69`、`cloud-config/src/cache.rs:24`、`windows-sandbox-rs/src/setup.rs:184-192` |

### 3.2 仓库级 `.codex/` → `.grevo/`

- **安全语义（重点）**：`codex-rs/protocol/src/permissions.rs:24` `PROTECTED_METADATA_CODEX_PATH_NAME = ".codex"`，`:626` 把项目根 `.codex/` 默认列为 agent 只读保护路径 ✅。改名必须与权限默认值同步，否则保护失效或误伤。
- 技能发现：仓库范围 `<repo>/.codex/skills`（`core-skills/src/loader.rs` 及 `loader_tests.rs:432`）。
- 配置写入/检测（external-agent-migration）：`repo/.codex/config.toml`、`.codex/hooks.json`、`.codex/agents`（`external-agent-migration/src/service.rs:502-607`、`detect/mod.rs:85,166,258`）。
- 插件清单约定：**`.codex-plugin/plugin.json`**（`core-plugins/src/plugin_bundle_archive.rs:62,65`、`manifest.rs:150`、`loader.rs:1177`、`utils/plugins/src/plugin_namespace.rs:125-208`）。第三方插件仓库已按此约定发布 → 建议**双读**（新目录优先，回落旧目录）。
- 本仓库自身的 `.codex/`（根目录）：是开发工具目录（20 个 agent 技能 + `.codex/environments/environment.toml`，其中 `name = "codex"`、Run 命令 `cargo ... --bin codex` ✅），当前**未被 gitignore**（git status 显示为未跟踪）。若产品约定改为 `.grevo`，此目录应同步改名。

### 3.3 其他含 codex 的路径约定

- 安装布局目录：`codex-package.json`、`codex-path/`、`codex-resources/`（`install-context/src/lib.rs:9-12` ✅；`scripts/install/install.sh:945-951`、`install.ps1:704-733`、`sdk/python-runtime` wheel 内容清单、`.github/scripts/build-codex-package-archive.sh`）。
- Windows 防火墙白名单：`/etc/codex/allowed_domains.txt`（`.devcontainer/init-firewall.sh:5`、`codex-cli/scripts/init_firewall.sh:6`）。
- 文件名约定：`.codex-remote-plugin-install.json`（`core-plugins/src/store.rs:23`）、`codex-package.json`（npm/PyPI 元数据）、`.codex-system-skills.marker`（§3.1）。

### 3.4 SQLite 数据库（补充盘点，✅ 已逐文件核对）

运行时共 **5 个库**，默认都在 `$CODEX_HOME` 下（`codex-rs/state/src/sqlite.rs:24-28`），可被 `CODEX_SQLITE_HOME`（`state/src/lib.rs:82`）或 config `sqlite_home`（`config/src/config_toml.rs:451-452`）整体重定向。除 state crate 外没有其他 sqlite 落点——`thread-store`、`agent-graph-store`、remote-control（`app-server-transport/.../enroll.rs`）、`core/src/rollout.rs`、doctor 全部复用这 5 个库。

| 库文件                    | 内容（主要表）                                                                                                                                                                                                    | 迁移目录                           |
|---------------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|------------------------------------|
| `state_5.sqlite`          | 线程索引/元数据（`threads`，含 `thread_source`、`cli_version`、`agent_nickname`、`preview` 等列）、`agent_jobs`、`remote_control_enrollments`、`external_agent_config_imports`、dynamic tools、thread spawn edges | `state/migrations/`（0001–0040+）  |
| `logs_2.sqlite`           | 日志、反馈日志体（`feedback_log_body`）                                                                                                                                                                           | `state/logs_migrations/`           |
| `goals_1.sqlite`          | 线程目标                                                                                                                                                                                                          | `state/goals_migrations/`          |
| `memories_1.sqlite`       | 记忆                                                                                                                                                                                                              | `state/memory_migrations/`         |
| `thread_history_1.sqlite` | 分页历史投影（`thread_turns`、`thread_items`、`thread_history_projection_state`）                                                                                                                                 | `state/thread_history_migrations/` |

改名视角的结论：

- **schema 与文件名都不含 "codex"**：全部迁移 `.sql` 文件（state 40 个 + logs 2 + goals 2 + memories 1 + thread_history 4）grep 零命中；表/列名（`threads`、`thread_items`、`logs`、`agent_jobs`…）无品牌词；库文件名尾部的数字（state_**5**、logs_**2**）是"架构纪元"号——破坏性 schema 变更时 bump 数字开新库（logs 已 1→2），不是品牌词。→ **改名不需要动数据库层**。
- **主目录迁移即完成数据迁移**：`~/.codex` → `~/.grevo` 时 5 个库随目录整体搬移即可；配套改 `CODEX_SQLITE_HOME` → `GREVO_SQLITE_HOME`；config 键 `sqlite_home` 本身无品牌、不变。
- **实操红线：不要修改历史迁移 `.sql` 文件**。迁移用 `sqlx::migrate!` 内嵌（`state/src/migrations.rs`），按版本号 + checksum 校验（每个库里的 `_sqlx_migrations` 表）；改历史文件会让旧库 checksum 不匹配直接打不开。品牌相关的任何 schema 变更必须以**新增迁移文件**的方式做。另有 `ignore_missing` 宽松策略允许旧二进制打开新库（`runtime_migrator`）。
- **值级别的品牌残留（仅 2 处）**：
  1. `threads.thread_source`（TEXT，migration 0030）：存 `SessionSource` 序列化串（`cli`/`vscode`/`exec`/`mcp`/`internal_*`/`subagent_*`，`protocol/src/protocol.rs:2839-2856`），枚举值本身无品牌；但 `Custom(String)` 接受任意来源串，旧行里可能存在 "codex"（读回走 `FromStr`，`thread_metadata.rs:496`，按字符串兼容即可）。
  2. `Product` 枚举（`chatgpt`/`codex`/`atlas`，serde alias `CHATGPT`/`CODEX`/`ATLAS`，`to_app_platform`，`protocol/src/protocol.rs:3795-3812`）：OpenAI 产品分类，随自建后端移除/替换（§5.1）；历史行旧值按字符串兼容读。
- **可选的"重新开库"决策**：若想 Grevo 时代从全新 schema 起步，可 bump 库文件名（`state_6.sqlite`…）——代价是旧线程索引/历史投影不可见（rollout JSONL 原文仍在 `sessions/`，但索引回填要重建，参考 migration `0008_backfill_state` 的机制；旧库文件不会被自动删除）。默认建议：**保留现有文件名，随目录迁移**——品牌上零损失，数据零风险。
- 周边：doctor 的线程盘点与损坏恢复（`cli/src/doctor/thread_inventory.rs`、`cli/src/state_db_recovery.rs`）只引用上述路径（损坏时备份挪开再重建）；相关 metric `codex.sqlite.*`、`codex.db.*` 已归入 §5.1 一并改 `grevo.*`。

---

## 4. 环境变量清单

### 4.1 Tier 1 —— 契约级，建议保留或需架构决策

| 变量                                 | 位置                                                | 说明                                                                                                                                |
|--------------------------------------|-----------------------------------------------------|-------------------------------------------------------------------------------------------------------------------------------------|
| `CODEX_SANDBOX`                      | `core/src/spawn.rs:25` ✅                            | 沙箱标记（seatbelt）；**AGENTS.md 明令禁止修改相关代码**，测试靠它早退                                                              |
| `CODEX_SANDBOX_NETWORK_DISABLED`     | `core/src/spawn.rs:20,79` ✅                         | 同上；`lmstudio`、`ollama` 客户端依赖它跳过网络调用                                                                                 |
| `CODEX_ESCALATE_SOCKET`              | `shell-escalation/src/unix/escalate_protocol.rs:11` | 提权 IPC socket fd 传递（父子进程契约）                                                                                             |
| `CODEX_THREAD_ID`                    | `protocol/src/shell_environment.rs:6`               | 注入子 shell；**fork 的 fm/license 依赖它判断"Codex 会话内"**（`fm/license/src/license.rs:332-341`）——改名需与 license 逻辑同批决策 |
| `CODEX_INTERNAL_ORIGINATOR_OVERRIDE` | `login/src/auth/default_client.rs:42` ✅             | 外部 harness 可能设置                                                                                                               |
| `CODEX_HOME`                         | `utils/home-dir/src/lib.rs:14` ✅                    | → `GREVO_HOME`（B 类，带迁移）                                                                                                      |

### 4.2 Tier 2 —— 产品级（随品牌一起改：`CODEX_` → `GREVO_`）

`CODEX_API_KEY`、`CODEX_ACCESS_TOKEN`、`CODEX_REMOTE_AUTH_TOKEN`、`CODEX_AUTH`、`CODEX_AUTHAPI_BASE_URL`、`CODEX_REFRESH_TOKEN_URL_OVERRIDE`、`CODEX_REVOKE_TOKEN_URL_OVERRIDE`、`CODEX_CA_CERTIFICATE`、`CODEX_GITHUB_TOKEN`、`CODEX_CONNECTORS_TOKEN`、`CODEX_SQLITE_HOME`、`CODEX_APP_SERVER_MANAGED_CONFIG_PATH`、`CODEX_APP_SERVER_DISABLE_MANAGED_CONFIG`、`CODEX_APP_SERVER_LOGIN_ISSUER`、`CODEX_APP_SERVER_DEV_OPEN_APP_URL`、`CODEX_APP_SERVER_REMOTE_CONTROL_DISABLED`、`CODEX_EXEC_SERVER_URL`、`CODEX_LOG`、`CODEX_CLOUD_TASKS_MODE/BASE_URL/FORCE_INTERNAL`、`CODEX_OSS_BASE_URL/PORT`、`CODEX_ANALYTICS_EVENTS_CAPTURE_FILE`、`CODEX_TUI_SESSION_LOG_PATH`、`CODEX_TUI_RECORD_SESSION`、`CODEX_TUI_DISABLE_KEYBOARD_ENHANCEMENT`、`CODEX_TUI_ROUNDED`、`CODEX_ROLLOUT_TRACE_ROOT`、`CODEX_CODE_MODE_HOST_PATH`、`CODEX_APPLY_GIT_CFG`、`CODEX_BUILD_COMMIT`、`CODEX_MANAGED_PACKAGE_ROOT`、`CODEX_MANAGED_BY_PNPM/NPM/BUN`、`CODEX_DOCTOR_DISABLED_MCP_TOKEN`、`CODEX_PERMISSION_PROFILE`、`CODEX_NETWORK_*`（proxy 系列 6 个）、`CODEX_WINDOWS_SANDBOX_PROXY_PORTS`、`CODEX_WEBSOCKET_*`、`CODEX_SKIP_BWRAP_BUILD`、`CODEX_BWRAP_SOURCE_DIR`。
安装器（非 Rust）：`CODEX_RELEASE`、`CODEX_NON_INTERACTIVE`、`CODEX_INSTALLER_USE_RELEASES_OPENAI_COM`、`CODEX_INSTALL_DIR`（`scripts/install/install.sh:5-18`）。
npm/Rust 双端契约：`CODEX_MANAGED_BY_*` + `CODEX_MANAGED_PACKAGE_ROOT`（`codex-cli/bin/codex.js:182-188` ↔ `install-context/src/lib.rs:109-113`）——两端必须同批改。

### 4.3 Tier 3 —— 仅测试/CI（量大，机械替换即可）

`CODEX_TEST_*`（约 8 个）、`CODEX_WINDOWS_PROXY_TEST_*`、`CODEX_EXEC_SERVER_*`、`CODEX_MCP_*_TEST_*`、`CODEX_AUTH_SYSTEM_PROXY_TEST_*`、`CODEX_SNAPSHOT_POLICY_MARKER`、`CODEX_CI`、`CODEX_BIN`、`CODEX_BAZEL_TEST_SKIP_FILTERS`（含 `workspace_root_test_launcher.*.tpl` 模板）、`CODEX_WINE_EXEC_TEST_BINARY`、`CODEX_ARGUMENT_COMMENT_LINT_SKIP_RUSTUP_SHIMS` 等。

### 4.4 Fork 自有环境变量

- `fm/license/src/license.rs:15-49` ✅：`FMSH_CODEX_LIC_FEATURE / _VERSION / _DISPLAY_NAME / _HOSTNAME / _TEST_BYPASS / _TEST_FORCE_LOST`。若改 `FMSH_GREVO_LIC_*`，需同步 license 发放/服务端配置及 `release/build-fm-cargo.sh:233-241`。

---

## 5. 对外协议与线上标识（走网络/跨进程的部分）

### 5.1 发给模型后端 / 后端的标识（已定：不接 OpenAI 官方服务）

前提确认后，本节所有项不再受 OpenAI 契约约束——统一改为"随自建后端一起重命名/定义"；OpenAI 专有链路（ChatGPT 登录、residency、OpenAI 模型目录）列为移除/重写候选：

| 项                           | 现值                                                                                                                                                                                                                                                                                                                                                                                          | 位置                                                                                                | 处置建议                                                                                                                                       |
|------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------------------|
| originator                   | `"codex_cli_rs"`                                                                                                                                                                                                                                                                                                                                                                              | `login/src/auth/default_client.rs:40` ✅（HTTP 头 `originator`，UA 亦由它派生 ：159-183）            | B：改 `grevo_cli_rs`；仅本机旧会话回放会带旧值，按普通字符串兼容即可                                                                           |
| 一方 originator 名单         | `codex-tui`、`codex_vscode`、`codex_atlas`、`codex_chatgpt_desktop`、`starts_with("Codex ")`                                                                                                                                                                                                                                                                                                  | `default_client.rs:148-157`                                                                         | A：替换为 grevo 值；旧值识别仅在还支持读取本机旧会话时保留                                                                                     |
| residency 头                 | `x-openai-internal-codex-residency`                                                                                                                                                                                                                                                                                                                                                           | `default_client.rs:43` ✅                                                                            | A：OpenAI 专有，移除或随自建后端重定义                                                                                                         |
| `x-codex-*` 请求头           | `x-codex-installation-id`、`x-codex-turn-state`、`x-codex-turn-metadata`、`x-codex-parent-thread-id`、`x-codex-window-id`、`x-codex-ws-stream-request-start-ms`、`x-codex-beta-features`                                                                                                                                                                                                      | `core/src/client.rs:143-153,1896` ✅；`rollout-trace/src/inference.rs:28`                            | B：语义保留（sticky routing、installation id 等），名字随自建后端改为 `x-grevo-*`；**客户端与后端必须同批切换**                                |
| 后端 URL 路径                | `https://chatgpt.com/backend-api/codex`（`model-provider-info/src/lib.rs:38`）、`/api/codex/*`（`backend-client/src/client/rate_limit_resets.rs:82-107`、`cloud-tasks-client/src/http.rs:296,580`）、`/codex/analytics-events/events`（`analytics/src/client.rs:116`）、`/codex/device`（`login/src/device_code_auth.rs:174`）、`chatgpt.com/codex/open-app`（`login/src/success_page.rs:7`） | —                                                                                                   | A：整批替换为自建后端地址（grevo 路径）；`chatgpt.com`/`auth.openai.com` 域名删除                                                              |
| OAuth / 登录                 | `CLIENT_ID = "app_EMoamEEZ73f0CkXaXp7hrann"`（`login/src/auth/manager.rs:1448`）、issuer `auth.openai.com`、回调端口 `1455`、`missing_codex_entitlement` 错误码、`login/src/assets/success.html` ChatGPT 页                                                                                                                                                                                   | `login/src/server.rs:57,965-976`                                                                    | A：走自建认证（fork 已有 fm-license / LMClient / UKey 体系）；ChatGPT/OAuth 登录链路整体移除或替换为自有认证                                   |
| JWT claim                    | `chatgpt_plan_type`、`chatgpt_account_id`、`https://api.openai.com/profile                                                                                                                                                                                                                                                                                                                    | auth` claim 键                                                                                      | `login/src/token_data.rs:34-96`                                                                                                                | A：随自有 token 格式重定义 |
| 模型 provider 与目录         | 预设 `https://api.openai.com/v1`、`chatgpt.com/backend-api/`（`model-provider-info/src/lib.rs:257` 等）；`models-manager/models.json` 的 `gpt-5*-codex` slug 与指令模板                                                                                                                                                                                                                       | —                                                                                                   | A：接入自建模型服务时整体重写；`gpt-5*-codex` 等 OpenAI 模型 slug 不再适用（保留 `CODEX_OSS_*`→`GREVO_OSS_*` 的 ollama/lmstudio 本地模型通道） |
| 遥测 metric 名               | `codex.*`（`cloud-config/src/metrics.rs:3-5`、`rollout/src/compression.rs:836-846`、`state/src/lib.rs:85-91`）、meter 名 `"codex"`（`otel/src/metrics/client.rs:45`）                                                                                                                                                                                                                         | —                                                                                                   | B：新前缀 `grevo.*`，自有 dashboard 直接用新名                                                                                                 |
| OTEL service.name            | 默认跟随 originator（`core/src/otel_init.rs:80-84`）；`codex_mcp_server`（`mcp-server/src/lib.rs:56`）、`codex-app-server`（`app-server/src/lib.rs:136`）                                                                                                                                                                                                                                     | —                                                                                                   | A：替换为 grevo 值                                                                                                                             |
| originator tag 白名单        | `codex_desktop`、`codex-app-server`、`codex_mcp_server`、`codex_cli_rs`、`codex-tui`、`codex_vscode`、`codex_exec`、`codex-cli`、`codex_sdk_ts`、`codex-app-server-sdk`…                                                                                                                                                                                                                      | `otel/src/metrics/tags.rs:14-26`                                                                    | A：替换为 grevo 枚举（后端是自己的，无需保留旧值）                                                                                             |
| analytics 上报               | `/codex/analytics-events/events`、`product_surface: "codex"`                                                                                                                                                                                                                                                                                                                                  | `analytics/src/client.rs:116`、`reducer.rs:2595`                                                    | B：路径与取值随自有后端一起定义                                                                                                                |
| exec-server relay            | Noise prologue `b"codex-exec-server-relay-noise/v1"`                                                                                                                                                                                                                                                                                                                                          | `exec-server/src/noise_channel.rs:35`                                                               | B：两端同发，可改 `grevo-exec-server-relay-noise/v1`（版本错峰风险）                                                                           |
| 会话文件内 `originator` 字段 | 旧 rollout 持久化了 `codex_cli_rs` 等                                                                                                                                                                                                                                                                                                                                                         | `protocol/src/protocol.rs:3057-3067`；resume 时 `core/src/thread_manager.rs:1449-1491` 优先复用旧值 | C：解析保持字符串兼容（只影响本机旧文件，随 §10.3-2 的迁移决策走）                                                                             |

### 5.2 MCP 边界

- 对外 serverInfo：`Implementation::new("codex-mcp-server", ...).with_title("Codex")`（`mcp-server/src/message_processor.rs:241` ✅）→ `grevo-mcp-server` / `Grevo`。
- 对外工具：`"codex"`（"Run a Codex session..."）与 `"codex-reply"`（`mcp-server/src/codex_tool_config.rs:118-123,237-241`）→ `grevo` / `grevo-reply`（**IDE 侧按工具名调用，需同步插件**）。
- 作为客户端连外部 MCP 时的自报名：`"codex-mcp-client"` + title `Codex`（`codex-mcp/src/rmcp_client.rs:930`）。
- 内置 hosted-apps MCP：config 键 `mcp_servers.codex_apps`、工具名 `mcp__codex_apps__*`（`core/src/mcp_tool_exposure.rs:26`、`protocol/src/models.rs:2850-2862`）→ `mcp__grevo_apps__*`（会进模型上下文，属行为变更）。
- app-server：`clientInfo.name = "codex-tui"`（`tui/src/lib.rs:405,569` ✅）；`app-server/src/request_processors/thread_processor.rs:16` 的 `CODEX_TUI_CLIENT_NAME` 与 `tools/src/tool_discovery.rs:5` 的客户端判断需同步；`request_plugin_install.rs:138` 对 `"codex-tui"` 做等值判断（行为门控）。

### 5.3 子进程 / IPC 边界

见 §2.2、§2.4 与 §4.1（arg0 哨兵、`--codex-home` 参数（`windows-sandbox-rs/src/wrapper.rs:23`）、`CODEX_ESCALATE_SOCKET`、`/tmp/codex-linux-sandbox-proxy-*`、线程名 `codex-main`、TUI↔IDE socket 路径 `~/.codex/ipc/ipc.sock`）。

---

## 6. 用户可见文案（UI / CLI 输出）

> 已确认与 OpenAI 无关 → 文案里的 "OpenAI"/"ChatGPT" 品牌表述（欢迎横幅、登录页、插件市场 "OpenAI Curated"、`chatgpt.com`/`developers.openai.com` 链接等）不是"改名"而是**移除/替换**问题：换成 Grevo 自有表述，或连同对应功能一起下线。

### 6.1 TUI（`codex-rs/tui/src/`）

| 位置                                                                                        | 现文案                                                                                                                                                                                                             |
|---------------------------------------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `onboarding/welcome.rs:96-98` ✅                                                             | "Welcome to **Codex**, OpenAI's command-line coding agent"                                                                                                                                                         |
| `history_cell/session.rs:331-333,403` ✅                                                     | 会话标题 `">_ OpenAI Codex (v…)"`（也写入会话日志）                                                                                                                                                                |
| `status/card.rs:711-731`                                                                    | `/status` 头 `"OpenAI Codex (v…)"`、`"run codex login to use ChatGPT"`                                                                                                                                             |
| `slash_command.rs:87-140`                                                                   | `/init`、`/quit`、`/skills`、`/personality`、`/permissions`、`/logout` 描述里的 "Codex"                                                                                                                            |
| `chatwidget/notifications.rs:50`                                                            | "Codex wants to edit {}"                                                                                                                                                                                           |
| `chatwidget/windows_sandbox_prompts.rs:241-398`                                             | "Set up the Codex agent sandbox..." 等一组                                                                                                                                                                         |
| `chatwidget/plugin_catalog.rs:73-857`                                                       | "OpenAI Curated" 系列标题                                                                                                                                                                                          |
| `chatwidget/model_popups.rs:50,225`、`rate_limits.rs:462`                                   | "OpenAI base URL is overridden..."、"…continue using codex"                                                                                                                                                        |
| `tooltips.rs:7-18`                                                                          | 公告抓取 URL `raw.githubusercontent.com/openai/codex/.../announcement_tip.toml` + 推广文案（"Try the Desktop app. Run 'codex app'…"）                                                                              |
| `update_prompt.rs:29`、`updates.rs:57-58`、`update_action.rs:44-61`、`npm_registry.rs:5,79` | 更新源：`github.com/openai/codex/releases/latest`、brew cask `codex`、npm `@openai/codex`、`chatgpt.com/codex/install.sh` → **必须指向 fork 自己的发布渠道**                                                       |
| `feedback_view.rs:34-570`                                                                   | 反馈链接 `github.com/openai/codex/issues`、`go/codex-feedback*`、`codex-logs.log`                                                                                                                                  |
| `onboarding/auth.rs:393-635`                                                                | 登录文案（"Sign in with ChatGPT to use Codex…"）                                                                                                                                                                   |
| 其余                                                                                        | `external_agent_config_migration*`、`model_migration.rs`、`bottom_pane/title_setup.rs`、`pets/catalog.rs`（宠物 "Codex"）、`app/history_ui.rs:81`（深链 `codex://threads/...`）、`session_archive_commands.rs:262` |

根目录 `announcement_tip.toml:13` ✅：面向用户的停版公告，含 `github.com/openai/codex` 链接。

### 6.2 CLI 与 exec

- `cli/src/main.rs:95` clap about `"Codex CLI"`；`:107-108` usage ✅；`:129-222` 各子命令描述（"Run Codex non-interactively." 等）；`:289-2737` 帮助/错误文案几十处（`codex doctor`、`codex login`、`codex mcp add` 等）；`plugin_cmd.rs:34-35` marketplace 名 `openai-bundled-alpha`、`openai-primary-runtime`。
- `exec/src/event_processor_with_human_output.rs:218` ✅：exec 模式横幅 `"OpenAI Codex v{VERSION}"`。
- `exec/src/cli.rs:12-34`：`"codex exec [OPTIONS] ..."` usage。
- `login.rs:38-449`：登录提示文案。

### 6.3 其他服务/binary 的用户可见串

- 登录成功页 `login/src/assets/success.html`（"Signed in to Codex"、"Open Codex"、深链 `codex://threads/new`）。
- `mcp-server/src/patch_approval.rs:61`："Allow Codex to apply proposed code changes?"。
- `chatgpt/src/chatgpt_client.rs:36-97`、`protocol/src/error.rs:94,684-706`、`core/src/session_rollout_init_error.rs:41-58`、`core/src/unified_exec/process_manager.rs:88`、`features/src/lib.rs:620,1070,1408`：错误/公告文案中的 "Codex"/"ChatGPT"。
- `responses-api-proxy/src/read_api_key.rs:136`、`lib.rs:38`（"Minimal OpenAI responses proxy"）。
- `doctor` 系列（`cli/src/doctor.rs` 及子模块）大量 "Codex"/npm `@openai/codex` 文案。
- JSON-RPC schema 标题 `"CodexAppServerProtocol(V2)"`（`app-server-protocol/src/export.rs:1075,1112`）。
- rate-limit payload 的 `limit_id: "codex"`（`app-server/src/outgoing_message.rs:831,852`；TUI 按 `eq_ignore_ascii_case("codex")` 匹配：`chatwidget/rate_limits.rs:213`、`status/rate_limits.rs:232`）——**客户端判定与服务端值要一起改**。

---

## 7. 模型可见提示词（改名会改变模型行为 + 快照联动）

> 本轮不改：提示词会改变模型自我认知与行为，后续单独设计后再处理。本节仅保留盘点，不代表本轮实施范围。

| 位置                                                                                                                                     | 内容                                                                                                                                                                                                                                                                                                           |
|------------------------------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `core/gpt_5_codex_prompt.md:1` ✅、"gpt-5.1-codex-max_prompt.md"、`gpt-5.2-codex_prompt.md`                                               | "You are **Codex**, based on GPT-5. You are running as a coding agent in the **Codex CLI**…"                                                                                                                                                                                                                   |
| `core/gpt_5_1_prompt.md:1,9`、`gpt_5_2_prompt.md`、`prompt_with_apply_patch_instructions.md:1,9`                                         | "You are GPT-5.1 running in the Codex CLI… Codex CLI is an open source project led by OpenAI…"、"…Codex refers to the open-source agentic coding interface…"                                                                                                                                                   |
| `core/templates/model_instructions/gpt-5.2-codex_instructions_template.md:1`                                                             | "You are Codex, a coding agent based on GPT-5…"                                                                                                                                                                                                                                                                |
| `models-manager/src/model_info.rs:18` + `models-manager/models.json` ✅                                                                   | `DEFAULT_PERSONALITY_HEADER` + **14 处** `"You are Codex…"` 指令模板；模型描述（"Codex-optimized…"）与 slug（`gpt-5-codex` 等，是 OpenAI 模型名，**不要改**）                                                                                                                                                  |
| `prompts/templates/realtime/backend_prompt.md:3`                                                                                         | "You are Codex, an OpenAI general-purpose agentic assistant…"                                                                                                                                                                                                                                                  |
| `protocol/src/protocol.rs:126`                                                                                                           | 用户消息前缀 `"## My request for Codex:"`（历史会话里的旧前缀仍需可解析）                                                                                                                                                                                                                                      |
| `core/src/realtime_context.rs:30`、`tui/src/goal_files.rs:18`、`memories/write/src/prompts.rs:84`、`core/src/guardian/prompt.rs:145-189` | "Startup context from Codex"、"Read the Codex goal objective file…"、"Consolidate Codex memories…"、"The Codex agent has requested…"                                                                                                                                                                           |
| `tui/src/inline_visualization.rs:27,243`                                                                                                 | 模型输出协议标记 `::codex-inline-vis{`（解析器同步）                                                                                                                                                                                                                                                           |
| 内置技能（进模型上下文）                                                                                                                 | 已删除 `imagegen`、`openai-docs` 两个 Skill；其余保留项包括 `skill-installer/agents/openai.yaml`、`plugin-creator/scripts/create_basic_plugin.py:68-69`（生成 "…in Codex." 文案） |

处置：本轮保持现状。后续单独设计模型提示词口径；若模型底座也弃用 GPT-5 系（换自建/其他模型），`gpt_5_*_prompt.md` 与 `models.json` 指令模板随模型目录整体重写（见 §10.3-9）。

---

## 8. crate / 包名与工作区结构

- **映射规则**：`codex-` → `grevo-`（包名与 lib 名同步，如 `codex-core` → `grevo-core`、lib `codex_core` → `grevo_core`）。涉及约 110 个 `[package].name`、`[workspace.dependencies]` 全部别名（`codex-rs/Cargo.toml:156-278`）、所有 `use`/extern crate 名与 BUILD.bazel 的 `crate_name`。
- **特例（目录名 ≠ 包名，rename 映射表要保留）**：`codex-utils-path` → 目录 `utils/path-utils`；`codex-agent-extension` → `ext/agent`；`codex-aws-auth` → `aws-auth`；`codex-chatgpt` → `chatgpt`；`codex-message-history` → `message-history`；`codex-windows-sandbox` → `windows-sandbox-rs`；`codex-agent-graph-store`（lib 名下划线不规则）。
- **fork 自有 crate**：`fm-license`、`fm-encrypted-skills`、`fm-product-policy`（依赖 codex-core 等，包名可保留或改 `grevo-*`，看内部命名策略）。`fm/license` 里的品牌默认值 `"Codex"`（`license.rs:173` ✅）、文案 `"Codex license is unavailable…"`（`:35`）要改。
- 元数据：仅 `execpolicy/Cargo.toml:6` 有 `description = "Codex exec policy: …"`；所有 crate 默认 `publish = true`（仅 `backend-client` 显式 false）——若打算发 crates.io 需要全量设 publish 策略。
- `[patch.crates-io]` 中 `openai-oss-forks/*`（`Cargo.toml:584-593`）是上游 fork 仓库地址，与品牌无关，保留。

---

## 9. 发行渠道与构建分发（对外身份，改名即"新包"）

### 9.1 npm（`codex-cli/` + `codex-rs/responses-api-proxy/npm` + `sdk/typescript`）

| 现名                                                              | 位置                                                                           |
|-------------------------------------------------------------------|--------------------------------------------------------------------------------|
| `@openai/codex`（bin: `codex`）                                   | `codex-cli/package.json:2,6-8` ✅                                               |
| 平台子包 `@openai/codex-{linux,darwin,win32}-{x64,arm64}`（6 个） | `codex-cli/bin/codex.js:16-23`、`codex-cli/scripts/build_npm_package.py:23-66` |
| `@openai/codex-responses-api-proxy`                               | `codex-rs/responses-api-proxy/npm/package.json:2` ✅                            |
| `@openai/codex-sdk`（导出类 `Codex`）                             | `sdk/typescript/package.json:2` ✅、`src/index.ts:29`                           |
| npm tarball 名 `codex-npm-*.tgz`、`codex-sdk-npm-*.tgz`           | `.github/workflows/rust-release.yml:1448-1549`                                 |

→ 已定不接 OpenAI：直接启用新身份（例如 `@<yourorg>/grevo` 系列 + bin `grevo`），无需为 `@openai/*` 旧名做兼容；`codex.js` 内 `CODEX_MANAGED_BY_*` 契约两端同批改。

### 9.2 PyPI（`sdk/python*`）

- `openai-codex`（import 包 `openai_codex`）、`openai-codex-cli-bin`（import 包 `codex_cli_bin`，wheel 内含 `codex-package.json`、`bin/codex`、`codex-resources/`、`codex-path/`）：`sdk/python/pyproject.toml:6,19` ✅、`sdk/python-runtime/pyproject.toml:6,37-43` ✅、`_runtime_setup.py:25-130`、`scripts/update_sdk_artifacts.py:26-63`、`.github/workflows/python-{sdk-release,runtime-build,runtime-release}.yml`。

### 9.3 安装器 / 更新通道 / 产物名

- `scripts/install/install.sh`：`BIN_PATH=$HOME/.local/bin/codex`、`CODEX_HOME_DIR=$HOME/.codex`、`STANDALONE_ROOT=$CODEX_HOME/packages/standalone`、下载资产 `codex-package-*.tar.gz`、`codex-package_SHA256SUMS`、`codex-npm-*.tgz`、RC 标记 `# >>> Codex installer >>>`、卸载 `@openai/codex`、`brew uninstall --cask codex`（:15-951）。`install.ps1` 对应 Windows 版。
- R2 更新通道：对象前缀 `codex/releases/<ver>`、`codex/channels/latest`、`codex/install.sh`、`https://releases.openai.com/codex`（`.github/scripts/publish_r2_release.py:6-30`、`r2-release.yml:22-45`）→ 已定不接 OpenAI：全部换为自有通道域名与 `grevo/` 前缀。
- 产物名：`codex-package-<triple>.tar.gz`、`codex-app-server-package-*`、`codex-<triple>.dmg`、`codex-symbols-*`、`codex-zsh`（`.github/workflows/rust-release.yml:340-1291`、`rust-release-zsh.yml`、`dotslash-config.json`、`dotslash-zsh-config.json`、macOS 签名 entitlements `codex*.entitlements.plist`）。
- Docker/AppImage：见 §11 fork release。

---

## 10. 兼容性红线与决策点（实施前必须拍板）

### 10.1 仍需旧值兼容的识别逻辑（范围已缩小）
- 旧会话回放：本机 `~/.codex` 下的旧 rollout 恢复时会带着 `codex_cli_rs` 等旧 originator（`core/src/thread_manager.rs:1449-1491` 复用持久化值）。若决定不做旧数据迁移（§10.3-2），此条可放弃；若迁移，旧值按普通字符串兼容即可，不再需要"一方 originator"语义。
- `PROTECTED_METADATA_CODEX_PATH_NAME`：若 `.grevo/` 为新约定，旧仓库里的 `.codex/` 仍应受保护（建议两个路径都保护）。
- `.codex-plugin/plugin.json`、`~/.codex`（迁移期双读）。
- 原"rate-limit `limit_id: "codex"` 匹配、originator 枚举保留旧值"两条随自建后端一次切清，无需兼容。

### 10.2 保留 / 需谨慎
- `NOTICE`、`docs/CLA.md`、LICENSE 中的 OpenAI 版权与 Apache-2.0 许可声明：**法律上必须保留**（Apache-2.0 §4 要求保留版权与归属声明），另加 Grevo 自身版权行即可。
- `CODEX_SANDBOX` / `CODEX_SANDBOX_NETWORK_DISABLED`：上游 AGENTS.md 第 8-10 行的禁令针对上游协作场景；该变量纯为内部父子进程契约（不含 OpenAI 依赖），**可以改名为 `GREVO_SANDBOX*`**，但必须：① 同批替换所有 set/read/测试 early-exit 点（`core/src/spawn.rs:20,25`、`sandboxing/mod.rs`、`cli/src/debug_sandbox.rs`、`lmstudio`、`ollama`、`shell-command` 测试等 10+ 处）；② 同步修订 AGENTS.md 对应条款；③ 一个原子提交完成。求稳的话保留旧名也不影响品牌（用户几乎不接触）。
- 裁决记录：原"`x-openai-*` 头、backend-api、OAuth client_id、JWT claim 不能动"的约束，随"不接 OpenAI 官方服务"的决定解除，转入 §5.1 的自建后端改造。

### 10.3 决策点清单
1. ~~后端策略~~ **已定（2026-09-04）**：不接 OpenAI/ChatGPT 官方服务，与 OpenAI 无关 → §5.1 全部按自建后端处理。
2. ~~`~/.codex` → `~/.grevo` 的用户数据迁移策略~~ **已定（2026-09-05）：不做自动迁移**。5 个 SQLite 库（§3.4）无品牌命名，需要旧数据时手动 `mv ~/.codex ~/.grevo` 即可全部保留。
3. `CODEX_SANDBOX*` 是否改名为 `GREVO_SANDBOX*`（技术上可行，见 §10.2；建议随 Phase 1 原子改）。
4. npm scope 与 PyPI 包名定名（新身份，无旧名兼容负担）。
5. MCP 工具名 `codex` → `grevo` 的时间点（配套 IDE 插件发版节奏）。
6. ~~提示词口径~~ **已定（2026-09-08）：本轮不改，后续单独设计**（见 §7）。
7. 更新通道（npm、releases URL、announcement 抓取 URL、brew 等）指向 fork 自有基础设施的具体地址。
8. fork license 环境变量 `FMSH_CODEX_LIC_*` 是否随之改名（涉及 license 服务端）。
9. 模型底座：若弃用 GPT-5 系模型，`models.json` 模型目录、`gpt_5_*_prompt.md`、`gpt-5*-codex` slug 需整体重写（`CODEX_OSS_*`→`GREVO_OSS_*` 的 ollama/lmstudio 本地通道可保留）。
10. ChatGPT 登录、cloud-tasks、analytics 后端这三块 OpenAI 专属功能：替换为自有实现，还是直接移除？

---

## 11. Fork 自有分发链（`release/`、`fm/`）

| 文件                                                      | codex 相关内容                                                                                                                                                        |
|-----------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `release/build-fm-cargo.sh` ✅                             | 产物 `dist/codex`、`codex-<ver>-x86_64.AppImage`、镜像 tag `codex:v…`、`cargo build -p codex-cli`、`/usr/local/bin/codex`、`ENTRYPOINT ["codex"]`、`FMSH_CODEX_LIC_*` |
| `release/build-fm.sh`                                     | `bazel build //codex-rs/cli:codex`、`FM_BUILD_SUFFIX`、镜像 tag、AppImage 名                                                                                          |
| `release/Dockerfile`、`Dockerfile.cargo` ✅（路径抽样）    | `/usr/local/bin/codex`、`ENTRYPOINT ["codex"]`、tag `codex:…`、`FM_BUILD_SUFFIX`                                                                                      |
| `release/BUILD.md`（中文构建指南）                        | 全篇 codex 路径/产物名/`FMSH_CODEX_LIC_*`（默认 display name "Codex"）                                                                                                |
| `codex-rs/cli/build.rs` + `cli/src/main.rs:101,965-971` ✅ | `FM_BUILD_SUFFIX` → 版本串 `<ver>-fm.rNNN-<hash>`（无品牌词，保留）                                                                                                   |
| `requirements.example.toml:69`                            | `audit_path = "/var/log/codex/encrypted-skills.log"`（代码回退值在 `core/src/config/mod.rs:713`，为 `/tmp/fm_skill_security_audit.log`，不含 codex）                  |
| `fm/license`                                              | §4.4、§8（display name 默认 `"Codex"`）                                                                                                                               |
| `fm/encrypted-skills`                                     | 仅测试引用 `~/.codex/skills`；`fm/product-policy` ✅ 无 codex 引用                                                                                                     |

---

## 12. 构建系统 / CI / 开发工具链（内部，需与 crate/二进制改名联动）

- **Bazel**：`MODULE.bazel:1` `module(name = "codex")` ✅（模块名决定 lock/缓存键，改名会使 `MODULE.bazel.lock` 全量重算）；`codex-rs/cli/BUILD.bazel:7,25,29`（`crate_name = "codex_cli"`、target `codex`、`codex-help`）；`//codex-rs/cli:release_binaries`（`bazel/platforms/release_binaries.bzl`）；`defs.bzl:185` 宏名 `codex_rust_crate`、`:290-291,349-624` 的 `"codex-rs/"` 字符串运算与 remap-path-prefix；`bazel/rules/e2e_benchmark.bzl:7` `codex_e2e_benchmark`。
- **justfile**（根）✅：`c`/`codex` 别名、`cargo run --bin codex*`、`bazel run //codex-rs/cli:codex`、`-p codex-core --bin codex-write-config-schema` 等约 15 处。
- **根 package.json / pnpm-workspace** ✅：`codex-monorepo`、workspace 成员 `codex-cli`、`codex-rs/responses-api-proxy/npm`、`sdk/typescript`；`write-hooks-schema -p codex-hooks`。
- **CI（.github/workflows，62 文件）**：`rust-release.yml`（全量产物名/矩阵）、`r2-release.yml`、`python-*` 三个发布流、`sdk.yml`（`//codex-rs/cli:codex`、`CODEX_EXEC_PATH`）、`issue-deduplicator/labeler/translator`（配套 `.github/codex/` agent 自动化目录）、`bazel.yml`/`rust-ci*`（路径引用）、`Dockerfile.bazel` + `rbe.bzl`（RBE 镜像 `mbolin491/codex-bazel`）、`.bazelrc:97` `REPO_URL=https://github.com/openai/codex.git`。
- **签名/打包脚本**：`.github/scripts/build-codex-package-archive.sh`、`build_codex_package.py`、`scripts/stage_npm_packages.py`、macOS entitlements、code-sign actions 默认 binaries 列表。
- **devcontainer**：`devcontainer(.secure).json`（名字 "Codex"、volume `codex-home-* → /home/vscode/.codex`、装的是**上游** `@openai/codex@0.121.0`——与 OpenAI 决裂后应改为安装 Grevo 自身产物或移除）、`post_install.py`（`~/.codex` + 写 gitignore `.codex/`）、`init-firewall.sh`（`/etc/codex/`）。
- **docs/**（根）：`install.md`、`contributing.md`、`getting-started.md`、8 个跳转 `developers.openai.com/codex/*` 的 stub——要么重写成 Grevo 文档，要么删除。
- **外部模型名**：`gpt-5-codex`、`gpt-5.1-codex-mini` 等 slug 是 OpenAI 模型 ID——与 OpenAI 决裂后随模型目录整体替换（§10.3-9），不再保留。

---

## 13. 仓库本地开发工具（非产品，随团队习惯改）

| 目录                                         | 性质                             | codex 内容                                                                                                                              |
|----------------------------------------------|----------------------------------|-----------------------------------------------------------------------------------------------------------------------------------------|
| `.codex/`（根，未跟踪）                      | 本仓库编码 agent 技能 + 环境定义 | `environments/environment.toml`（`name = "codex"`、`--bin codex`）✅、20 个技能（`codex-bug`、`codex-pr-body`、`codex-issue-digest` 等） |
| `.omp/`（未跟踪）                            | 另一个 CLI 工具的镜像技能集      | openspec 系列命令                                                                                                                       |
| `openspec/`（已跟踪）                        | fork 规格文档                    | spec 正文描述 Codex 行为（文案级）                                                                                                      |
| `analysis/`（已 gitignore，`.gitignore:1`）✅ | 内部分析笔记                     | `CODEX_STORAGE_STRUCTURE_ANALYSIS.md` 等 4 个文件名 + 正文                                                                              |
| `fm-docs/`                                   | fork 内部文档（12 篇）           | 正文引用 codex-rs 路径                                                                                                                  |
| `AGENTS.md`（根）                            | 开发规范                         | "Crate names are prefixed with codex-"、CODEX_SANDBOX 条款等——改名落地时需同步修订（尤其 §10.2 若保留 CODEX_SANDBOX 则该条款原文保留）  |

> 注意：`.gitignore` 目前**没有** `.codex/`、`.omp/` 条目 ✅（仅第 1 行 `analysis`、第 21 行 `codex-cli/README.md`）；这两个目录当前以未跟踪状态出现在 git status 中。若不打算提交，应补 gitignore。

---

## 14. 建议实施顺序（每步可独立验证）

1. **Phase 0 决策**：过一遍 §10.3 决策点（后端、迁移策略、发布身份、CODEX_SANDBOX 豁免）。
2. **Phase 1 可执行文件名**（最小可感知单元）：`[[bin]] name` 全量 + arg0 哨兵/常量 + clap bin_name/usage/补全 + Bazel target/justfile/CI 矩阵 + 安装器路径。验证：`cargo build` + `just fmt` + `just test -p codex-cli`（本阶段后改包内引用）+ release 脚本干跑。
3. **Phase 2 crate 包名**（纯机械、量大）：`codex-*` → `grevo-*`（包名/lib 名/别名/use/BUILD label），随后 `just bazel-lock-update` 重生成 `MODULE.bazel.lock`、Cargo.lock；按 AGENTS.md 跑 `just fix -p <crate>`。
4. **Phase 3 目录与 env 约定**：`~/.codex`→`~/.grevo`、`CODEX_HOME`→`GREVO_HOME`、`.codex/`→`.grevo/`（含权限保护路径双保护）、`.codex-plugin/`、`.codex-system-skills.marker`、`codex-package.json` 等 + 用户数据迁移逻辑 + npm/安装器 env 契约两端同步。
5. **Phase 4 文案与提示词**：§6、§7 全量替换 + `cargo insta` 重生 38 个快照 + `just write-config-schema`、`just write-app-server-schema`、hooks schema 重生 + 模型行为回归（用真实会话抽查提示词效果）。
6. **Phase 5 对外协议**：originator/UA/MCP serverInfo 与工具名/clientInfo/metric 前缀改为 grevo 值；`x-codex-*` 头与后端 URL 与自建后端**同批切换**；旧值仅本机旧会话字符串兼容。
7. **Phase 6 发行渠道**：npm scope、PyPI 包、安装器、R2/更新 URL、docker tag/AppImage、release workflow 全量改名，发布冒烟（安装→登录→会话→恢复旧会话→更新检查）。
8. **Phase 7 fork 收尾**：`fm/license` 展示名/文案/env（与 license 服务端联动）、`release/*` 脚本、`requirements.example.toml` audit_path、本地 `.codex/`→`.grevo/` 开发目录与 AGENTS.md 修订。

---

## 15. 最小化改名方案（只动用户交互侧 + 配置侧）

> 目标：用户敲的命令、看到的品牌、放配置的目录全部变成 Grevo；**其余（110 个 crate 名、29 个辅助 bin、全部其他 `CODEX_*` 环境变量、originator、MCP 名、SQLite、文件格式、npm/PyPI）一律不动**。这是 §14 全量方案的一个可独立交付子集，后续任何阶段都可以在此之上继续。
>
> **实施状态（2026-09-05）**：P0 已在 `grevo` 分支实施完毕。实施时用户追加决定：**不做 `~/.codex` → `~/.grevo` 自动迁移**——新版直接使用 `~/.grevo`，旧目录原样保留（想搬数据可手动 `mv ~/.codex ~/.grevo`）；`GREVO_HOME > CODEX_HOME` 的环境变量优先级保留。
>
> **扩展实施（同日，用户追加要求）**：crate 包名/库名与环境变量也已全量改名——
> - **crate**：全部 129 个 `codex-*` 包名 → `grevo-*`（含 lib 名、`[workspace.dependencies]` 别名、全部 `use`/路径引用、BUILD.bazel `crate_name`、Cargo.lock）。**crate 目录名未改**（`codex-rs/`、`codex-api/` 等 bazel 标签与工作区成员路径保持原样，避免雪崩）。
> - **环境变量**：全部 `CODEX_*` → `GREVO_*`（含 `CODEX_SANDBOX*`、`CODEX_HOME`、`CODEX_THREAD_ID`、测试变量），并同步 npm shim（`codex.js`）、TS/Python SDK、安装器、CI 工作流、`.bazelrc`/测试启动模板、AGENTS.md 条款。两个刻意保留：home-dir 的 `CODEX_HOME` 旧名回落（兼容既有脚本），以及本机旧会话等落盘数据不受影响。
> - **保持不变**：辅助 bin 名（`codex-tui`、`codex-linux-sandbox` 等，与 arg0 哨兵值/CI 矩阵/安装布局强耦合）、app-server schema fixture 文件名（`codex_app_server_protocol*.json` 硬编码，python SDK 引用）、MCP 工具名/originator 等线上标识（待自建后端阶段处理）。
>
> **目录名实施（同日追加）**：全部 codex 命名目录已改名——`codex-rs/` → **`grevo-rs/`**、6 个 crate 目录（`codex-api`、`codex-backend-openapi-models`、`codex-client`、`codex-experimental-api-macros`、`codex-home`、`codex-mcp` → `grevo-*`）、npm 包装目录 `codex-cli/` → `grevo-cli/`、打包脚本包 `scripts/codex_package` → `scripts/grevo_package`、本地开发目录 `.codex/` → `.grevo/`。所有 BUILD 标签（`//grevo-rs/...`）、CI 路径、`defs.bzl` 路径运算、AGENTS.md、devcontainer、VSCode 配置同步更新。仍保留 codex 名的：辅助 bin 名、少量文件名（`codex.js`、`codex.ts`、`build_codex_package.py`、`docs/codex_mcp_interface.md` 等文件级命名，未在本轮范围）、`.github/codex/` 内部 agent 自动化目录、法律文件中的 OpenAI 归属。本清单中更早的 `codex-rs/...` 路径引用为历史记录。

### 15.1 P0 必改清单（≈30 个文件，估 <400 行 diff）

#### A. 可执行文件 `codex` → `grevo`（含分发链涟漪）

| #  | 文件                                                                                                                                                          | 改动                                                                                         |
|----|---------------------------------------------------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------|
| A1 | `codex-rs/cli/Cargo.toml:9`                                                                                                                                   | `[[bin]] name = "codex"` → `"grevo"`                                                         |
| A2 | `codex-rs/cli/src/main.rs:107-108`                                                                                                                            | `bin_name = "codex"`、`override_usage` 里的 `codex` → `grevo`                                |
| A3 | `codex-rs/cli/src/main.rs:2664`、`:1116`                                                                                                                      | 补全生成 `let name = "codex"`、`try_parse_from(["codex", …])` → `grevo`                      |
| A4 | `codex-rs/cli/BUILD.bazel:25` 及引用 `//codex-rs/cli:codex` 的 4 处（根 `justfile:121,125`、`release/build-fm.sh:136`、`.github/workflows/sdk.yml`）          | bazel target `codex` → `grevo`（`crate_name = "codex_cli"` 是内部 lib 名，可不动）           |
| A5 | `release/build-fm-cargo.sh`（`dist/codex`、`/usr/local/bin/codex`、AppImage 名）、`release/build-fm.sh`、`release/Dockerfile`、`release/Dockerfile.cargo`      | 产物/容器内路径/`ENTRYPOINT ["codex"]`/镜像 tag → grevo                                      |
| A6 | 根 `justfile:14-36`、`.codex/environments/environment.toml`（Run 命令 `--bin codex`）、根 `package.json` scripts                                             | `--bin codex` → `--bin grevo`（开发工具联动，机械）                                          |

> arg0 机制**不需要动**：`apply_patch`/`codex-linux-sandbox`/`codex-execve-wrapper` 等 helper 名是内部契约（两端同仓库），保留原样即可工作。

#### B. 目录与配置约定 `.codex` → `.grevo`

| #  | 文件                                                                                                                                                          | 改动                                                                                                                               |
|----|---------------------------------------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------------------------------|
| B1 | `codex-rs/utils/home-dir/src/lib.rs:13-60`                                                                                                                    | 默认目录 `push(".codex")` → `push(".grevo")`；env 改**双读**：`GREVO_HOME` 优先 → `CODEX_HOME` 回落 → `~/.grevo`                    |
| B2 | —                                                                                                                                                             | **已决定不做自动迁移**（用户确认）：直接使用 `~/.grevo`；旧数据由用户手动搬移或放弃                                              |
| B3 | `codex-rs/protocol/src/permissions.rs:24,626`                                                                                                                 | 默认只读保护路径：加入 `.grevo`，**保留 `.codex`**（旧仓库/迁移期双保护）                                                          |
| B4 | `codex-rs/config/src/loader/mod.rs:934`                                                                                                                       | 项目级配置目录 `repo/.codex` → `repo/.grevo`（config.toml / hooks.json / agents / skills 的仓库作用域全从这里走）                   |
| B5 | `codex-rs/external-agent-migration/src/detect/mod.rs:85,166,258`、`service.rs:502-607`                                                                        | 写入/检测路径 `.codex` → `.grevo`（读回落 `.codex` 可选）                                                                          |
| B6 | `codex-rs/core-plugins/src/manifest.rs:150`、`plugin_bundle_archive.rs:62,65`、`utils/plugins/src/plugin_namespace.rs:125-208`                                | 插件清单：读 `.grevo-plugin/plugin.json` 优先、**回落 `.codex-plugin/plugin.json`**；写入只用新路径                                |
| B7 | `codex-rs/skills/src/lib.rs:27`                                                                                                                               | `.codex-system-skills.marker` → `.grevo-system-skills.marker`                                                                      |
| B8 | `codex-rs/state/src/bin/logs_client.rs:151`                                                                                                                   | 独立硬编码的 `.codex` → `.grevo`（更稳的做法：改为复用 `find_codex_home`）                                                        |
| B9 | `codex-rs/config/src/loader/layer_io.rs:20`、`requirements.example.toml:69`                                                                                   | 系统配置层 `/etc/codex/…` → `/etc/grevo/…`、审计日志 `/var/log/codex/…` → `/var/log/grevo/…`（若 fork 使用该层；不用可后置）       |
| B10 | 根 `.gitignore`（可选）、本地 `.codex/` 开发目录                                                                                                             | 本仓库自己的 `.codex/` 目录改名 `.grevo/`（或至少加入 .gitignore）                                                                 |

#### C. 用户可见文案最小集

| #   | 文件                                                                                                                        | 现文案 → 新文案                                                                                                                  |
|-----|-----------------------------------------------------------------------------------------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------|
| C1  | `tui/src/onboarding/welcome.rs:96-98`                                                                                       | "Welcome to Codex, OpenAI's command-line coding agent" → "Welcome to Grevo"                                                      |
| C2  | `tui/src/history_cell/session.rs:331-333,403`                                                                               | "OpenAI Codex (v…)" → "Grevo (v…)"                                                                                               |
| C3  | `tui/src/status/card.rs:711-731`                                                                                            | /status 标题同步                                                                                                                 |
| C4  | `exec/src/event_processor_with_human_output.rs:218`                                                                         | exec 横幅 "OpenAI Codex v…" → "Grevo v…"                                                                                         |
| C5  | `cli/src/main.rs:95`                                                                                                        | clap about "Codex CLI" → "Grevo CLI"                                                                                             |
| C6  | `tui/src/slash_command.rs:87-140`                                                                                           | 6 条命令描述里的 Codex → Grevo                                                                                                   |
| C7  | `codex-rs/fm/license/src/license.rs:35,173`                                                                                 | "Codex license is unavailable…"、display 默认名 "Codex" → Grevo                                                                  |
| C8  | `announcement_tip.toml` + `tui/src/tooltips.rs:7`                                                                           | 公告内容去掉 openai 链接；抓取 URL 换自有地址（或先置空停用）                                                                    |
| C9  | 更新检查 URL：`cli/src/doctor/updates.rs:25`、`tui/src/updates.rs:57-58`、`tui/src/npm_registry.rs`、`update_action.rs`     | 指向 openai/npm/brew 的地址换自有或接受 404（fork 走自有分发时可直接禁用更新提示）                                               |
| C10 | TUI insta 快照（约 38 个）                                                                                                  | `just test -p codex-tui` → `cargo insta accept -p codex-tui` 重生                                                                |

### 15.2 P1 可选加强（默认不做，想要再做）

- **模型自称**：`core/gpt_5_codex_prompt.md` 等基础 prompt 与 `models-manager/models.json` 14 处 "You are Codex" → "You are Grevo"（删 "led by OpenAI"）。代价：`core/src/session/tests.rs:1307` 断言同步 + 模型行为回归。不改不影响功能，只是模型自我介绍仍是 Codex。
- 长尾错误文案（`protocol/src/error.rs`、`chatgpt` crate、`core/src/session_rollout_init_error.rs` 等）里的 Codex。
- MCP serverInfo/工具名 `codex-mcp-server`/`codex` → grevo（自己 IDE 对接时再改，两端同批）。
- `fm/license` 的 `FMSH_CODEX_LIC_*` 环境变量改名（需 license 服务端配合）。

### 15.3 明确保留不动（最小方案的边界）

| 保留项                                                                                  | 理由                                                                                    |
|------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------|
| ~~110 个 `codex-*` crate/包名、lib 名~~                                                  | **已于同日全量改为 `grevo-*`**（见上方扩展实施记录）                                    |
| `codex-tui`、`codex-exec`、`codex-linux-sandbox` 等 29 个辅助 bin 名 + arg0 哨兵         | 内部/开发者可见；日志文件名 `codex-tui.log` 随主目录迁移，无品牌冲突                    |
| `CODEX_SANDBOX*` 及其余全部 `CODEX_*` 环境变量                                          | 内部契约；仅新增 `GREVO_HOME` 一个新变量（优先级高于 `CODEX_HOME`）                     |
| originator `codex_cli_rs`、UA、MCP serverInfo/工具名、`x-codex-*` 头                     | 后端是自己的，按 §15.2 节奏再切；现阶段保留零成本                                       |
| 5 个 SQLite 库、rollout 格式、`sessions/` 等子目录名                                     | §3.4 已确认无品牌词，随主目录迁移数据全保留                                             |
| schema 标题 `CodexAppServerProtocol*`、`codex-package.json` 安装布局、npm/PyPI           | fork 未走这些渠道；待 §14 Phase 5/6 再说                                                |

### 15.4 兼容垫片汇总（P0 内置，共 5 条）

1. `GREVO_HOME` > `CODEX_HOME` > `~/.grevo`；
2. `.grevo-plugin/` 读优先、`.codex-plugin/` 读回落，写入只新路径；
3. 权限默认保护 `.grevo/` 与 `.codex/` 双目录；
4. ~~首启自动迁移 `~/.codex`~~ **已决定不做**——`~/.grevo` 全新开始，旧数据手动处理；
5. 不迁移时旧会话/配置不会被读取；如需找回，手动 `mv ~/.codex ~/.grevo` 即可（目录结构与文件格式零改动，SQLite 见 §3.4）。

### 15.5 验证清单

`cargo build -p codex-cli` → `just fmt` → `just test -p codex-tui`（快照重生）→ 若动 B3/B4 跑 `cargo test -p codex-protocol codex-config` → 手工冒烟：`grevo` 启动（观察旧数据自动迁移）、`config.toml`/技能/插件加载、`~/.grevo/sessions` 新会话写入、旧 `~/.codex` 已不在（被 rename）、`grevo --help` 与补全输出、`release/build-fm-cargo.sh` 出 `dist/grevo` + AppImage 可执行。

---

## 附录 A：高价值"一次改干净"映射总表

| 域                                      | Codex 现值                                                           | Grevo 新值                                                            | 类别                       |
|-----------------------------------------|----------------------------------------------------------------------|-----------------------------------------------------------------------|----------------------------|
| 主二进制                                | `codex`                                                              | `grevo`                                                               | A                          |
| 主目录                                  | `~/.codex` / `CODEX_HOME`                                            | `~/.grevo` / `GREVO_HOME`                                             | B（迁移）                  |
| 仓库约定目录                            | `.codex/`（skills/config/hooks/agents）                              | `.grevo/`                                                             | B（双保护旧目录）          |
| 插件清单                                | `.codex-plugin/plugin.json`                                          | `.grevo-plugin/plugin.json`                                           | B（双读）                  |
| originator                              | `codex_cli_rs`                                                       | `grevo_cli_rs`                                                        | B（本机旧会话字符串兼容）  |
| MCP serverInfo / 工具                   | `codex-mcp-server` / `Codex` / 工具 `codex`                          | `grevo-mcp-server` / `Grevo` / `grevo`                                | A（同步 IDE）              |
| clientInfo                              | `codex-tui`                                                          | `grevo-tui`                                                           | B                          |
| env 前缀                                | `CODEX_*`（Tier 2）                                                  | `GREVO_*`                                                             | B（`CODEX_SANDBOX*` 除外） |
| 提示词自称                              | "You are Codex … Codex CLI"                                          | "You are Grevo … Grevo CLI"                                           | A（快照/行为回归）         |
| npm/PyPI                                | `@openai/codex*` / `openai-codex*`                                   | `@<yourorg>/grevo*` / `grevo*`                                        | A（新身份）                |
| 日志/标记                               | `codex-tui.log`、`.codex-system-skills.marker`、`codex-package.json` | `grevo-tui.log`、`.grevo-system-skills.marker`、`grevo-package.json`  | A/B                        |
| 后端 URL 与 `x-codex-*`/`x-openai-*` 头 | `chatgpt.com/backend-api/codex`、`x-codex-turn-state` 等             | 自建后端地址 + `x-grevo-*`（客户端与后端同批切换）；OpenAI 专有项移除 | B/A                        |
| ChatGPT 登录 / OAuth                    | `auth.openai.com`、`app_EMoam…`、端口 1455、JWT `chatgpt_*` claim    | 自有认证（fm-license / LMClient / UKey）或整块移除                    | A                          |

## 附录 B：改名后必须重生成/联动的生成物

`codex-rs/Cargo.lock`、`MODULE.bazel.lock`（`just bazel-lock-update`）、`pnpm-lock.yaml`、`core/config.schema.json`（`just write-config-schema`）、app-server-protocol/hooks schema fixtures（`just write-app-server-schema`）、TUI insta 快照（`cargo insta accept -p codex-tui`，约 38 个文件）、`blob-size-allowlist.txt` 与 `.gitattributes` 中 schema 路径、dotslash 配置、macOS entitlements 文件名、`docs/install.md` 的产物名。
