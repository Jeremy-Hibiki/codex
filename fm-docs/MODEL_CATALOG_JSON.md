# `model_catalog_json` / `models.json` 字段说明

本文对应当前 `codex-rs` 协议定义：`ModelsResponse` 与 `ModelInfo`。`model_catalog_json` 指向一个本地 JSON 文件；加载成功时 catalog 是权威数据源，App Server 使用 `StaticModelsManager`，不再请求远端 `/models`。

## 顶层结构

```json
{
  "models": [ { "...": "..." } ]
}
```

- `models`：必填，数组；空数组会被配置加载拒绝。
- 额外顶层字段会被 serde 忽略。
- 字段名都是 snake_case；错误大小写或错误枚举值会导致整个 catalog 解析失败。

## 字段总表

表中“必填”指没有 serde `default`，省略会导致反序列化失败。必填字段名加粗；`Option` 字段即使显示“必填”，通常也可用 `null` 显式表达空值。

| 字段 | 必填 | 默认/空值 | 摘要 |
|---|---:|---|---|
| **`slug`** | 是 | 无 | 模型匹配键。 |
| **`display_name`** | 是 | 无 | App Server/AHP/UI 的人类可读名称。 |
| **`description`** | 是 | `null` | UI 副标题。 |
| `default_reasoning_level` | 否 | `null` | 默认 reasoning effort。 |
| **`supported_reasoning_levels`** | 是 | 无 | 可选 reasoning effort 列表。 |
| **`shell_type`** | 是 | 无 | Shell 工具形态。 |
| **`visibility`** | 是 | 无 | 模型列表可见性。 |
| **`supported_in_api`** | 是 | 无 | API/custom provider 模式过滤。 |
| **`priority`** | 是 | 无 | 模型列表排序。 |
| `additional_speed_tiers` | 否 | `[]` | 已废弃的 speed tier 列表。 |
| `service_tiers` | 否 | `[]` | 可用服务档位。 |
| `default_service_tier` | 否 | `null` | UI 推荐服务档位。 |
| **`availability_nux`** | 是 | `null` | 可用性提示。 |
| **`upgrade`** | 是 | `null` | 推荐升级模型。 |
| **`base_instructions`** | 是 | 无 | 基础系统提示。 |
| `model_messages` | 否 | `null` | 模型专用消息与 personality 模板。 |
| `include_skills_usage_instructions` | 否 | `false` | 是否注入技能用法说明。 |
| `supports_reasoning_summary_parameter` | 否 | `true` | 是否发送 `reasoning.summary`。 |
| `default_reasoning_summary` | 否 | `"auto"` | 默认 reasoning summary。 |
| **`support_verbosity`** | 是 | 无 | 是否支持 verbosity。 |
| **`default_verbosity`** | 是 | `null` | 默认 verbosity。 |
| **`apply_patch_tool_type`** | 是 | `null` | 直接 `apply_patch` 工具选择。 |
| `web_search_tool_type` | 否 | `"text"` | hosted web search 输入类型。 |
| **`truncation_policy`** | 是 | 无 | 工具输出截断策略。 |
| **`supports_parallel_tool_calls`** | 是 | 无 | 是否允许并行工具调用。 |
| `supports_image_detail_original` | 否 | `false` | 图片 original detail 开关。 |
| `context_window` | 否 | `null` | 当前模型上下文窗口。 |
| `max_context_window` | 否 | `null` | 用户配置覆盖的上限。 |
| `auto_compact_token_limit` | 否 | `null` | 自动压缩阈值。 |
| `comp_hash` | 否 | `null` | 压缩兼容性标识。 |
| `effective_context_window_percent` | 否 | `95` | 有效上下文百分比。 |
| **`experimental_supported_tools`** | 是 | 无 | 实验工具启用列表。 |
| `input_modalities` | 否 | `["text","image"]` | 支持的输入模态。 |
| `supports_search_tool` | 否 | `false` | 命名空间 search 工具开关。 |
| `use_responses_lite` | 否 | `false` | Responses Lite 开关。 |
| `auto_review_model_override` | 否 | `null` | 自动评审替代模型。 |
| `tool_mode` | 否 | `null` | 工具模式选择。 |
| `multi_agent_version` | 否 | `null` | 兼容的 multi-agent 后端。 |
| `used_fallback_model_metadata` | 否 | `false` | 运行时内部字段；JSON 输入会被忽略。 |

## 身份与模型列表

### **`slug`**

必填字符串。模型元数据匹配键，不要求等于真实后端模型名，但精确相等是最稳配置。

匹配规则：

1. 先做最长前缀匹配：`requested_model.starts_with(candidate.slug)`，取最长候选。
2. 未命中且请求名形如 `namespace/model-name` 时，剥掉一个合法 namespace，再对 suffix 做最长前缀匹配。
3. 仍未命中则 fallback。

命中后，运行时把返回的 `ModelInfo.slug` 改写为实际请求模型名；因此请求发给模型服务的 `model` 值保持用户配置不变。

### **`display_name`**

必填字符串。链路是 `display_name -> ModelPreset.display_name -> App Server Model.displayName -> AHP SessionModelInfo.name -> VSCode LanguageModelChatMetadata.name`。

如果模型名没有匹配到 catalog 条目，fallback 元数据的 `display_name` 就是实际模型名，所以 UI 显示 slug 常常表示 fallback。

### **`description`**

`null` 或字符串；`null` 转换为空描述。App Server 原样返回给客户端，TUI/VSCode 通常作为模型说明或副标题展示。

### **`visibility`**

必填枚举：

- `list`：`show_in_picker = true`；`model/list includeHidden=false` 时可见。
- `hide`：`show_in_picker = false`；普通模型列表过滤掉，`includeHidden=true` 可返回。
- `none`：同样 `show_in_picker = false`；语义上表示 API 场景不可见项，但仍受 `supported_in_api` 过滤。

默认模型规则：按 `priority` 升序排序后，第一个 picker 可见模型被标记为默认；如果没有 picker 可见模型，则第一个模型成为默认。

### **`supported_in_api`**

必填布尔：

- `true`：API auth 与 ChatGPT auth 都保留。
- `false`：只在 ChatGPT/Codex backend auth 模式保留；普通 API/custom provider 模式会被过滤。

自托管 provider 常见错误是把该字段设为 `false`，导致模型不出现在列表。

### **`priority`**

必填整数，升序排序；数值越小越靠前。相同数值时保持排序前的相对顺序，默认项再按可见性规则从排序结果选择。

### `availability_nux`

`null` 或对象，对象只有必填字段 `message`。TUI 启动时按模型列表顺序选择第一个尚未达到展示次数上限的 NUX 并展示；展示次数持久化到 `[tui.model_availability_nux]`。

### `upgrade`

`null` 或对象：

- `model`：必填，升级目标模型 slug。
- `migration_markdown`：必填，迁移提示 Markdown。

转换为 App Server `ModelUpgradeInfo` 时，`modelLink` 与 `upgradeCopy` 固定为 `null`。

## Reasoning、verbosity 与服务档位

### `default_reasoning_level`

默认 `null`。请求构造时，显式 effort 优先；没有显式 effort 时才使用该值；仍为 `null` 则不发送 effort。

有效值：`none`、`minimal`、`low`、`medium`、`high`、`xhigh`、`max`、`ultra`。其他非空字符串解析为自定义 effort；空字符串解析失败。

### **`supported_reasoning_levels`**

必填数组，可为空。每项：

- `effort`：必填 reasoning effort。
- `description`：必填 UI 描述。

空数组表示 UI 不提供等级选择；fallback 元数据即为空数组。

### `supports_reasoning_summary_parameter`

默认 `true`：

- `true`：当 summary 配置不是 `none` 时，请求携带 `reasoning.summary`。
- `false`：请求不携带 `reasoning.summary`。

### `default_reasoning_summary`

默认 `"auto"`。有效值：`auto`、`concise`、`detailed`、`none`。显式 summary 配置优先；未显式配置时使用该默认值。

### **`support_verbosity`**

必填布尔：

- `true`：请求 verbosity 取用户显式配置，否则取 `default_verbosity`。
- `false`：不发送 verbosity。若用户设置了 verbosity，会记录“模型不支持”警告。

### **`default_verbosity`**

`null` 或枚举。有效值：`low`、`medium`、`high`。只有 `support_verbosity=true` 且用户未显式选择时才生效。

### `additional_speed_tiers`

默认空数组，已废弃。`supports_fast_mode()` 仍会检查是否包含 `"fast"`，但新配置应使用 `service_tiers`。

### `service_tiers`

默认空数组。每项必须包含 `id/name/description`。请求时，显式服务档位必须存在于 `service_tiers`，否则被过滤为 `null`；表示“使用默认档”的特殊请求值也不会发送。

### `default_service_tier`

默认 `null`。作为 UI 推荐档位，不自动作为请求参数；请求侧不会仅因该字段存在而发送默认档。

## 指令与技能

### **`base_instructions`**

必填字符串。常规 Responses 请求放入顶层 `instructions`。用户配置的 `base_instructions` 优先，并会清空 `model_messages.instructions_template/variables`。

### `model_messages`

默认 `null`。结构：

- `instructions_template`：字符串或 `null`；存在时优先于 `base_instructions`。
- `instructions_variables`：包含 `personality_default`、`personality_friendly`、`personality_pragmatic`。
- `approvals`：包含 `on_request`、`on_request_auto_review`、`never`、`unless_trusted`。
- `auto_review`：包含 `policy`、`policy_template`。
- `permissions`：包含 `danger_full_access`、`workspace_write`、`read_only`。

`instructions_template` 可包含 `{{ personality }}`。只有占位符存在且三个 personality 变量都存在时，`supports_personality` 才为 true。Personality 关闭或用户覆盖基础指令时会清空 instruction 模板与变量。

### `include_skills_usage_instructions`

默认 `false`：

- `true`：可用技能上下文包含技能使用方式说明。
- `false`：只列出技能，不额外注入技能用法说明。

## 工具调用与执行形态

### **`shell_type`**

必填枚举，最终形态还会受 feature 开关影响：

- `default`：等价归一到 `shell_command`，注册普通 shell command 工具。
- `local`：等价归一到 `shell_command`，注册普通 shell command 工具。
- `unified_exec`：优先注册 `exec_command` 与 `write_stdin`，并保留 dispatch-only legacy shell；如果 Unified Exec feature 不可用，则回退 `shell_command`。
- `disabled`：不注册 shell 执行工具。
- `shell_command`：注册普通 shell command 工具；当前 bundled catalog 的常用值。

全局 `ShellTool` feature 关闭时，无论 catalog 值是什么，最终都是 `disabled`。

### **`apply_patch_tool_type`**

`null` 或 `"freeform"`：

- `null`：不注册直接 `apply_patch` 工具。模型仍可通过 shell 形式执行 `apply_patch` 命令；shell handler 会拦截并应用补丁。
- `"freeform"`：注册 Responses custom/freeform 工具 `apply_patch`，模型直接输出补丁文本，不包 JSON。

对 vLLM 等只支持普通 named function calling、不支持 OpenAI Responses custom tool 的后端，应使用 `null`；`"freeform"` 可能导致工具调用协议无法闭环。

### **`truncation_policy`**

必填对象：

- `mode`：`bytes` 或 `tokens`。
- `limit`：必填整数。

工具输出超过 limit 时按模式截断。配置 `tool_output_token_limit` 时，会按模式换算为 bytes/tokens 并覆盖该 limit。

### **`supports_parallel_tool_calls`**

必填布尔：

- `true`：请求允许 `parallel_tool_calls`。
- `false`：请求强制不允许。

即使为 `true`，Responses Lite 也会把请求中的 `parallel_tool_calls` 强制设为 false。

### `tool_mode`

默认 `null`。有效值：

- `direct`：直接工具规划，覆盖 Code Mode feature。
- `code_mode`：启用 Code Mode；兼容的嵌套工具规格会增强为 Code Mode 形态。
- `code_mode_only`：只保留 Code Mode 兼容路径。
- `null`：由 Code Mode/Code Mode Only feature flags 推导。

未知字符串会被宽容地转为 `null`，不会导致 JSON 解析失败。

### **`experimental_supported_tools`**

必填数组，可为空。这是实验工具名白名单；当前源码已知 `test_sync_tool`。列表内不存在的实验工具不注册。它不是普通工具的通用启用机制。

### `use_responses_lite`

默认 `false`：

- `true`：请求走 Responses Lite：工具以 `AdditionalTools` developer 输入传递，hosted Responses tools 不发送，基础指令作为 developer message，添加 Responses Lite 标识头，reasoning context 使用 `all_turns`，并行工具调用强制关闭。
- `false`：常规 Responses：工具作为顶层 `tools` 发送，基础指令作为顶层 `instructions`。

### `auto_review_model_override`

默认 `null`。自动评审 fallback 模型选择时优先使用该 slug；为空则按可用模型列表选择。

## 搜索与多模态

### `web_search_tool_type`

默认 `"text"`：

- `text`：hosted web search 工具只接受文本查询。
- `text_and_image`：工具 schema 声明可搜索 `["text","image"]`。

该字段只影响 hosted web search 工具 schema。工具是否出现还要求 provider 声明支持 web search、web search mode 不是 `disabled`，且 standalone search 未接管。standalone search 可用时，hosted search 不会创建。

### `supports_search_tool`

默认 `false`：

- `true`：允许命名空间 search 工具路径；还要求 namespace tools 开启。
- `false`：不启用该路径。

它和 `web_search_tool_type` 是两条搜索路径：前者偏客户端/命名空间工具，后者描述 hosted Responses search。

### `input_modalities`

默认 `["text","image"]`。有效值可组合：

- `text`：接受文本输入。
- `image`：接受图片输入。
- `audio`：接受音频输入。

影响 prompt 构造时按模态过滤输入项。App Server/VSCode 也用是否包含 `image` 判断模型视觉能力。

### `supports_image_detail_original`

默认 `false`：

- `true`：`view_image` 允许请求 original 图像细节。
- `false`：不允许 original 细节。

## 上下文与压缩

### `context_window`

默认 `null`。当前模型上下文窗口；设置时优先于 `max_context_window`。运行时有效窗口为：

```text
resolved_context_window() * effective_context_window_percent / 100
```

### `max_context_window`

默认 `null`。作为用户配置 `model_context_window` 的上限。若用户设置 120k 而 max 为 100k，最终为 100k。仅设置 max、不设置 context 时，也作为解析上下文的 fallback。

### `effective_context_window_percent`

默认 `95`。把解析出的上下文窗口折算为 Codex 可用输入窗口。100 表示全额使用；过小会提前显示剩余量紧张并影响相关预算行为。

### `auto_compact_token_limit`

默认 `null`。若存在可解析上下文窗口，实际阈值为：

```text
min(auto_compact_token_limit, floor(resolved_context_window * 90 / 100))
```

到达该阈值或有效上下文硬上限时触发压缩；具体计数范围还受 `model_auto_compact_token_limit_scope` 影响。

### `comp_hash`

默认 `null`。模型切换时，如果旧模型与新模型 comp hash 不同，会先按旧模型上下文执行 inline compact，再进入新模型，避免压缩格式不兼容。`null` 表示不参与该强制兼容切换。

## 已废弃或兼容字段

以下字段出现在 bundled `models.json` 中，但当前 `ModelInfo` 没有声明，反序列化时会被忽略：

- `prefer_websockets`
- `minimal_client_version`
- `reasoning_summary_format`

不要依赖它们改变当前行为。

## 嵌套对象速查

### `supported_reasoning_levels[]`

| 字段 | 必填 | 说明 |
|---|---:|---|
| **`effort`** | 是 | 推理等级字符串。 |
| **`description`** | 是 | UI 中该等级的说明。 |

### `service_tiers[]`

| 字段 | 必填 | 说明 |
|---|---:|---|
| **`id`** | 是 | 档位标识，请求时会与它比较。 |
| **`name`** | 是 | UI 档位名。 |
| **`description`** | 是 | UI 档位说明。 |

### `availability_nux`

| 字段 | 必填 | 说明 |
|---|---:|---|
| **`message`** | 是 | 模型可用性提示文案。 |

### `upgrade`

| 字段 | 必填 | 说明 |
|---|---:|---|
| **`model`** | 是 | 升级目标模型 slug。 |
| **`migration_markdown`** | 是 | 升级提示 Markdown。 |

### `truncation_policy`

| 字段 | 必填 | 说明 |
|---|---:|---|
| **`mode`** | 是 | `bytes` 或 `tokens`。 |
| **`limit`** | 是 | 数值上限。 |

### `model_messages`

所有子字段都可省略或为 `null`。

| 字段 | 类型 | 行为 |
|---|---|---|
| `instructions_template` | string | 指令模板；可包含 `{{ personality }}`。 |
| `instructions_variables` | object | 三个 personality 字段都存在时 personality 才完整。 |
| `approvals` | object | 审批文案。 |
| `auto_review` | object | 自动评审文案。 |
| `permissions` | object | 权限文案。 |

## 加载与匹配行为

1. `model_catalog_json` 在配置加载阶段读取。
2. JSON 必须是完整 `{"models":[...]}`；任一模型反序列化失败会导致整个配置加载失败，不会单条丢弃。
3. 空数组失败。
4. 加载成功后 provider 创建 `StaticModelsManager`，远端 `/models` 不会成为该 catalog 的来源。
5. App Server `model/list` 将 catalog 转为 `ModelPreset`：`slug -> model/id`，`display_name -> displayName`。
6. 运行时模型元数据查找使用最长前缀匹配；命中后 `ModelInfo.slug` 改写为实际请求模型名。
7. 未命中时 fallback：`display_name = 模型名`、`visibility = none`、`priority = 99`、默认基础提示、无 reasoning 选项、`apply_patch_tool_type = null`、无并行工具调用，并记录 `Unknown model ... fallback model metadata`。

## 最小可用示例

```json
{
  "models": [
    {
      "slug": "deepseek-v4-flash-0731",
      "display_name": "DeepSeek V4 Flash 0731",
      "description": "Self-hosted DeepSeek V4 Flash model",
      "supported_reasoning_levels": [
        { "effort": "low", "description": "Low" },
        { "effort": "medium", "description": "Medium" },
        { "effort": "high", "description": "High" }
      ],
      "default_reasoning_level": "medium",
      "shell_type": "shell_command",
      "visibility": "list",
      "supported_in_api": true,
      "priority": 1,
      "availability_nux": null,
      "upgrade": null,
      "base_instructions": "You are Grevo.",
      "support_verbosity": false,
      "default_verbosity": null,
      "apply_patch_tool_type": null,
      "truncation_policy": { "mode": "tokens", "limit": 10000 },
      "supports_parallel_tool_calls": true,
      "experimental_supported_tools": [],
      "context_window": 1048576,
      "max_context_window": 1048576
    }
  ]
}
```

对 vLLM 这类不支持 Responses custom/freeform tool 的后端，保持 `apply_patch_tool_type: null`，让模型通过 shell 命令形式执行 `apply_patch`。
