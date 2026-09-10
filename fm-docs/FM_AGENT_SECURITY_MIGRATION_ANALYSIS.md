# fm-agent-security 插件迁移到 Codex 可行性分析报告

> 源插件：`opencode/.worktrees/feat/skill-protect/packages/opencode/src/plugin/fm-agent-security`
> 目标平台：`openai/codex` 插件系统
> 日期：2026-07-30

---

## 一、fm-agent-security 插件核心能力

该插件实现 **加密技能隔离 (Encrypted Skill Isolation)**，核心目标是防止主 Agent 直接读取解密后的 Skill 明文内容。

### 安全威胁覆盖

| 威胁类别       | 具体场景                             | 防护机制                     |
| -------------- | ------------------------------------ | ---------------------------- |
| Skill IP 窃取  | 主 Agent 直接读取解密后的 Skill 文件 | 解密→Token→SubAgent 执行     |
| Prompt 注入    | 对 executor 的 jailbreak 攻击        | 3 层 XML 防护 prompt         |
| 脚本泄露       | bash/stdout 中的源码泄露             | Tokenize + 过滤              |
| 跨会话污染     | Token 缓存被其他会话利用             | FIFO 64/会话隔离             |
| 数据外泄       | 通过 write/webfetch/export 导出      | Intent 检测 + 输出红化       |
| 目录结构探测   | grep/glob 发现目录                   | 工具调用拦截                 |
| 资源泄漏       | 解密目录未清理                       | 会话结束自动清理             |
| 符号链接绕过   | symlink 指向敏感路径                 | bwrap 沙箱 + 白名单          |
| 压缩时明文暴露 | compaction 时暴露密钥                | 安全上下文注入               |
| 并发冲突       | 多任务同时委托                       | FIFO 多槽 + PendingDelegates |
| 越权委托       | 非主 Agent 创建 executor             | AgentType 检查               |
| 入站意图违规   | export/jailbreak 意图                | Intent 检测器                |

### 8 种 Hook 实现

| Hook 类型                              | 文件                    | 功能                                                                     |
| -------------------------------------- | ----------------------- | ------------------------------------------------------------------------ |
| `tool.execute.before`                  | `hooks/tool-execute.ts` | 拦截工具调用：jailbreak 注入、工具拦截 (bash/grep/glob)、flowID 注入     |
| `tool.execute.after`                   | 同上                    | 结果 tokenization：bash 结果 token 化、read 结果 token 化、task 结果清理 |
| `chat.message`                         | `hooks/chat-message.ts` | 消息红化：SKILL-EXEC flowID 验证、入站意图检测                           |
| `event`                                | `hooks/event.ts`        | 会话生命周期：executor 注册/清理                                         |
| `experimental.chat.messages.transform` | `hooks/chat-message.ts` | 消息转换：Token→明文重水化                                               |
| `experimental.session.compacting`      | `hooks/compacting.ts`   | 压缩时安全上下文注入                                                     |
| `config`                               | `plugin.ts`             | 配置加载                                                                 |
| `dispose`                              | `plugin.ts`             | 清理                                                                     |

### 核心组件

```
fm-agent-security/
├── hooks/
│   ├── tool-execute.ts        # 工具拦截/结果 tokenization ★
│   ├── skill-tool.ts          # Skill 委托/解密/红化 ★
│   ├── chat-message.ts        # 消息红化/意图检测 ★
│   └── event.ts               # 会话生命周期
├── decryption.ts              # 解密流程 (Zip Slip 保护)
├── rehydrator.ts              # Token↔明文转换 (FIFO 64/会话)
├── intent_detector.ts         # 意图检测 (关键字+模型)
├── path-guard.ts              # 路径守卫 (bwrap 沙箱)
├── hardware_key.ts            # 硬件密钥验证
├── agents/                    # 加密 Skill Executor System Prompt
├── registry/                  # 注册表 (executor/pending/blocked/redaction)
├── audit.ts                   # JSONL 审计日志
├── constant.ts                # FORBID_SKILL_JAILBREAK_PROMPT
└── plugin.ts                  # 8 hooks 绑定 + 初始化
```

---

## 二、Codex Hook 接口 vs OpenCode Hook 接口对比

### 通信方式

| 维度      | OpenCode                                       | Codex                                     |
| --------- | ---------------------------------------------- | ----------------------------------------- |
| 语言      | **TypeScript** (直接调用 JS 函数)              | **Shell 命令** (JSON stdin/stdout)        |
| 调用方式  | 同步函数调用 `hookFn(payload)`                 | 异步 Shell 进程，stdin→stdout             |
| Hook 定义 | `registerHook('tool.execute.before', handler)` | JSON: `hooks/tool-execute.json` + command |
| 配置      | `.jsonc` 配置文件                              | `hooks.json` + `hooks.state` 信任管理     |

### Hook Payload 对比

**OpenCode** 的 tool.execute.before 事件（TypeScript 对象）：

```typescript
interface ToolExecuteBeforePayload {
  session: Session;
  tool: Tool;
  args: Record<string, unknown>;
  sessionId: string;
  turnId?: string;
  // 丰富的 Session 对象可以直接访问
}
```

**Codex** 的 pre_tool_use 事件（JSON stdin）：

```json
{
  "tool_name": "execute_bash",
  "tool_input": { "cmd": "cat /path/to/skill" },
  "tool_use_id": "toolu_...",
  "session_id": "sess_...",
  "cwd": "/workspace",
  "model": "codex-gpt5",
  "permission_mode": "default"
}
```

### Hook Result 对比

**OpenCode** 的返回值（TypeScript 对象）：

```typescript
return {
    shouldBlock: true,
    blockReason: "Skill access prohibited",
    additionalContext: [...],
    updatedInput: {...}
};
```

**Codex** 的返回值（JSON stdout）：

```json
{
    "decision": "block",
    "reason": "Skill access prohibited",
    "hookSpecificOutput": {
        "hookEventName": "PreToolUse",
        "permissionDecision": "deny",
        "permissionDecisionReason": "Skill access prohibited",
        "additionalContext": "...",
        "updatedInput": {...}
    }
}
```

> **源码验证**：输出结构定义于 `codex-rs/hooks/src/schema.rs:130-135, 254-267`，使用 `#[serde(rename_all = "camelCase")]` + `#[serde(deny_unknown_fields)]`

---

## 三、迁移可行性评估

### 3.1 直接可迁移的能力

| 能力                                  | 迁移方式           | 复杂度 | 说明                                               |
| ------------------------------------- | ------------------ | ------ | -------------------------------------------------- |
| **工具拦截 (pre_tool_use)**           | 改写为 Shell 脚本  | 低     | 匹配工具名，检查 `tool_input`，返回 `decision: block` |
| **结果 tokenization (post_tool_use)** | 改写为 Shell 脚本  | 低     | 接收 `tool_response`，输出 `additional_context`    |
| **会话生命周期 (session_start/end)**  | 改写为 Shell 脚本  | 低     | 标准 Hook 接口，payload 包含 session_id            |
| **工具白名单/黑名单**                 | hooks.json matcher | 低     | Codex 原生支持 `matcher` 正则/管道匹配             |
| **审计日志**                          | Shell 脚本内嵌     | 低     | 用 `jq` 或 `python3 -c` 写 JSONL                   |
| **Intent 检测 (关键字版)**            | Shell 脚本内嵌     | 中     | 用 grep/awk 正则扫描工具名/参数                    |
| **Token 缓存**                        | 临时文件 + 会话名  | 中     | 用 `mktemp -d` 创建会话隔离目录                    |

### 3.2 需要重构的能力

| 能力                           | 当前实现                             | 迁移方案                                                  | 复杂度 |
| ------------------------------ | ------------------------------------ | --------------------------------------------------------- | ------ |
| **加密解密 (Zip)**             | `fflate` JS 库                       | **Python 脚本** (ZipFile + cryptography) 或 **Go 小工具** | 高     |
| **硬件密钥验证**               | `hardware_key.ts` (env var mock)     | 保留 env var 检查 + 添加硬件绑定                          | 中     |
| **bwrap 沙箱包装**             | `path-guard.ts`                      | 直接调用 `bwrap` 命令 (Codex 已内置 bubblewrap)           | 中     |
| **Token↔明文转换**            | `rehydrator.ts` (FIFO 内存)          | 临时文件 (会话隔离目录)                                   | 中     |
| **PendingDelegates**           | JS Map + Set                         | Shell 变量或临时文件                                      | 低     |
| **Redaction RegExp**           | JS RegExp 编译                       | `sed`/`awk`/Python re                                     | 低     |
| **Executor Registry**          | JS Map                               | 文件/共享内存                                             | 低     |
| **加密 Skill Executor Prompt** | `agents/encrypted-skill-executor.md` | 直接复用 (System Prompt 文本)                             | 无     |
| **Jailbreak 防护 Prompt**      | `constant.ts` (XML 3-tier)           | 直接复用                                                  | 无     |

### 3.3 不可直接迁移的能力

| 能力                                       | 原因                                   | 替代方案                                              |
| ------------------------------------------ | -------------------------------------- | ----------------------------------------------------- |
| **`experimental.chat.messages.transform`** | Codex 无此 hook 事件                   | 通过 `post_tool_use` 模拟 + `additional_context` 注入 |
| **`event` hook (session.created/deleted)** | Codex 有 `session_start`/`session_end` | 用这两个事件替代                                      |
| **直接 TypeScript 函数调用**               | Codex 只支持 Shell 命令                | 全部转为 Shell/Python                                 |
| **Session 对象直接访问**                   | Codex 只传 JSON 字段                   | 需通过额外文件/环境传递上下文                         |
| **异步并行处理**                           | JS async/await → Shell 同步            | Shell 后台进程 + `wait` 或 `jq` 同步处理              |

---

## 四、推荐迁移架构

### 4.1 Codex Plugin 目录结构

```
fm-agent-security/
├── .codex-plugin/
│   └── plugin.json              # Manifest: name, hooks, skills
├── hooks/
│   ├── hooks.json               # Hook 路由配置 (Codex 格式)
│   ├── pre_tool_use/
│   │   ├── tool-execute.sh      # 主拦截逻辑 (bash + jq)
│   │   └── skill-tool.sh        # Skill 委托检查
│   ├── post_tool_use/
│   │   ├── result-tokenize.sh   # 结果 tokenization
│   │   └── redact.sh            # 输出红化
│   ├── session_start/
│   │   └── init-executor.sh     # Executor 注册 + 目录初始化
│   └── session_end/
│       └── cleanup.sh           # 会话清理
├── tools/
│   ├── decrypt-skill.py         # Python 加密 Skill 解包
│   ├── token-cache.py           # Token 缓存管理
│   ├── intent-detector.py       # 意图检测 (关键字+模型)
│   └── path-guard.sh            # 路径守卫 + bwrap 包装
├── agents/
│   └── encrypted-skill-executor.md  # Executor System Prompt (复用)
├── config.schema.json           # 配置 Schema (可复用)
└── README.md                    # 文档
```

### 4.2 hooks.json 配置示例

```json
{
  "description": "fm-agent-security: Encrypted Skill Isolation",
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "execute_bash",
        "hooks": [
          {
            "type": "command",
            "command": "bash {{plugin_root}}/hooks/pre_tool_use/tool-execute.sh",
            "timeout": 30,
            "statusMessage": "fm-agent-security: checking tool execution"
          }
        ]
      },
      {
        "matcher": "read_file|glob|grep",
        "hooks": [
          {
            "type": "command",
            "command": "bash {{plugin_root}}/hooks/pre_tool_use/skill-tool.sh",
            "timeout": 15,
            "statusMessage": "fm-agent-security: checking for skill access"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "execute_bash",
        "hooks": [
          {
            "type": "command",
            "command": "bash {{plugin_root}}/hooks/post_tool_use/result-tokenize.sh",
            "timeout": 30
          }
        ]
      }
    ],
    "SessionStart": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "bash {{plugin_root}}/hooks/session_start/init-executor.sh",
            "timeout": 15
          }
        ]
      }
    ],
    "SessionEnd": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "bash {{plugin_root}}/hooks/session_end/cleanup.sh",
            "timeout": 15
          }
        ]
      }
    ]
  }
}
```

### 4.3 核心逻辑伪代码示例

```bash
# hooks/pre_tool_use/tool-execute.sh
#!/usr/bin/env bash
# 接收 JSON stdin (Codex pre_tool_use payload)

# 读取输入
read -r payload
tool_name=$(echo "$payload" | jq -r '.tool_name')
tool_input=$(echo "$payload" | jq -r '.tool_input')
session_id=$(echo "$payload" | '.session_id')

# 检查工具是否被拦截
case "$tool_name" in
    execute_bash)
        # 检查是否涉及 skill 路径
        if echo "$tool_input" | grep -qE 'skills/|\.codex-skill|decrypt'; then
            echo '{"decision":"block","reason":"Skill access prohibited","hookSpecificOutput":{"hookEventName":"PreToolUse","permissionDecision":"deny","permissionDecisionReason":"Skill access prohibited"}}'
            exit 0
        fi
        ;;
    read_file|glob|grep|rg)
        # 阻止直接读取 skill 文件
        if echo "$tool_input" | grep -qE 'skills/|\.codex-skill'; then
            echo '{"decision":"block","reason":"File read prohibited for skill paths"}'
            exit 0
        fi
        ;;
esac

# 工具不在白名单中，放行
echo '{"decision":"approve"}'
exit 0
```

---

## 五、迁移复杂度矩阵

| 模块                   | TypeScript → Shell/Python           | 复杂度 | 工作量估算 |
| ---------------------- | ----------------------------------- | ------ | ---------- |
| **tool-execute.ts**    | 304 行 TS → Shell + jq + Python     | ★★★★★  | 2-3 天     |
| **skill-tool.ts**      | 委托逻辑 → Shell + 加密工具         | ★★★★★  | 3-5 天     |
| **chat-message.ts**    | 消息处理 → `post_tool_use` 模拟     | ★★★☆☆  | 1-2 天     |
| **decryption.ts**      | JS fflate → Python zipfile + crypto | ★★★★☆  | 2-3 天     |
| **rehydrator.ts**      | 内存 FIFO → 临时文件 + 会话名       | ★★★☆☆  | 1 天       |
| **intent_detector.ts** | JS → Python (关键字保留，模型需改)  | ★★★☆☆  | 1-2 天     |
| **path-guard.ts**      | JS → Shell + bwrap                  | ★★★☆☆  | 1 天       |
| **hardware_key.ts**    | env var 检查 → 保持                 | ★☆☆☆☆  | 半天       |
| **registry/**          | Map/Set → 文件/变量                 | ★★☆☆☆  | 1-2 天     |
| **audit.ts**           | JSONL 缓冲 → Shell 日志             | ★★☆☆☆  | 半天       |
| **agent prompt**       | 复用                                | ☆☆☆☆☆  | 0          |
| **config.schema**      | 复用                                | ☆☆☆☆☆  | 0          |

**总计：约 2-3 周**

---

## 六、核心差异与风险

### 6.1 架构差异

| 差异点        | OpenCode                | Codex                                        | 影响                                             |
| ------------- | ----------------------- | -------------------------------------------- | ------------------------------------------------ |
| **执行模型**  | 同步 JS 函数，共享内存  | 异步 Shell 进程，stdin/stdout                | **高**：需将所有状态改为外部存储 (文件/共享目录) |
| **Hook 定义** | 注册式 `registerHook()` | 声明式 `hooks.json`                          | **中**：需重新编写路由配置                       |
| **配置**      | `.jsonc` + env vars     | `hooks.json` + `hooks.state` + `config.toml` | **低**：配置格式需转换                           |
| **信任机制**  | 无 (OpenCode 插件系统)  | SHA256 `trusted_hash` 验证                   | **中**：需为每个脚本计算 hash                    |
| **并发控制**  | JS EventLoop + async    | Shell 进程 + FIFO                            | **中**：需引入文件锁或命名管道                   |

### 6.2 安全风险

1. **Shell 注入**：Codex hook 命令执行通过 shell，需严格参数化 (用 `jq` 或 Python 解析 JSON 而非 `eval`)
2. **权限提升**：Shell 进程权限等同于 Codex 进程，需确保不会意外逃逸
3. **状态不一致**：多进程并发时临时文件可能竞争，需引入 `flock` 或命名管道

### 6.3 性能考量

- OpenCode 是同步函数调用 (<1ms)
- Codex 是 fork+exec Shell 进程 (约 10-50ms/次)
- 每个工具调用可能触发 2-3 个 hook (pre + post + intent)
- **估算**：每次工具调用增加 ~100ms 延迟，对日常使用可接受

---

## 七、结论

### 可行性：**可行，但需要显著重构**

| 评估维度     | 结论                                                                                      |
| ------------ | ----------------------------------------------------------------------------------------- |
| **功能覆盖** | Codex hook 接口覆盖 fm-agent-security 需求的 **80%+** (PreToolUse, PostToolUse, SessionStart, SessionEnd, UserPromptSubmit, PreCompact, PostCompact 可对应) |

> **源码验证**：11 种 Hook 事件定义于 `codex-rs/hooks/src/lib.rs:20-31`
| **安全性**   | Shell 模型增加了攻击面，但通过 jq/Python 严格解析和 SHA256 信任可缓解           |
| **工作量**   | 约 2-3 周，核心复杂度在加密解密和 Token 管理                                    |
| **性能**     | Hook 延迟从 ~0 增加到 ~100ms/调用，可接受                                       |
| **维护**     | Shell/Python 替代 TypeScript 增加了跨平台兼容性挑战 (Windows 需额外处理)        |

### 推荐实施路径

1. **Phase 1** (基础): 重写 `tool-execute.sh` (工具拦截) + `session_start/end` 生命周期
2. **Phase 2** (核心): 实现 `decrypt-skill.py` + `skill-tool.sh` (加密 Skill 委托)
3. **Phase 3** (增强): 实现 `intent-detector.py` + `redact.sh` (意图检测 + 输出红化)
4. **Phase 4** (完善): 实现 `path-guard.sh` (bwrap 沙箱) + `cleanup.sh` (会话清理)

### 关键依赖

Codex 环境中需确保以下工具可用：

- `jq` (JSON 解析)
- `python3` (加密解密、意图检测)
- `bwrap` (沙箱)
- `flock` (并发锁)
- `mktemp` (会话隔离目录)
