# Codex 上下文管理系统分析

> 源码锚点：codex-rs/core/src/context_manager/, codex-rs/core/src/compact*.rs, codex-rs/core/src/context/, codex-rs/context-fragments/src/fragment.rs
> 更新时间：2026-07-30

---

## 1. 提示词注入机制

### 1.1 ContextualUserFragment Trait

所有上下文片段通过 `ContextualUserFragment` trait 定义：

```rust
// codex-rs/context-fragments/src/fragment.rs:22-65
pub trait ContextualUserFragment {
    fn role(&self) -> &'static str;                          // "user" | "developer"
    fn requires_separate_message(&self) -> bool { false }    // 是否独立消息
    fn markers(&self) -> (&'static str, &'static str);       // 开始/结束标记
    fn body(&self) -> String;                               // 内容体
    fn type_markers() -> (&'static str, &'static str)       // 类型标记
    
    fn render(&self) -> String {                            // 完整渲染
        let (start, end) = self.markers();
        format!("{}{}{}", start, self.body(), end)
    }
    
    fn into(self) -> ResponseItem where Self: Sized;         // 转换为 ResponseItem
}
```

### 1.2 主要提示词注入点

#### 1.2.1 Base Instructions（基础指令）

```rust
// codex-rs/core/src/client.rs:868-885
if !prompt.base_instructions.text.is_empty() {
    prefix.push(ResponseItem::Message {
        id: None,
        role: "developer".to_string(),
        content: vec![ContentItem::InputText {
            text: prompt.base_instructions.text.clone(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    });
}
```

**配置层级**（优先级从低到高）：
```toml
# codex-rs/core/src/config/mod.rs:3828-3838
[config]
instructions = "全局指令"                    # 默认值
model_instructions_file = "/path/to/file"  # 文件覆盖
base_instructions = "直接覆盖"              # 最高优先级
```

#### 1.2.2 Developer Instructions（开发者指令）

```rust
// codex-rs/core/src/config/mod.rs:683-684
pub developer_instructions: Option<String>,  // 独立 developer 消息
```

**特殊处理**：
- Subagent fork 时会替换父 agent 的 developer instructions
- V2 multi-agent 下使用 `subagent_developer_instructions` 替换

```rust
// codex-rs/core/src/agent/control/spawn.rs:603-625
let (subagent_developer_instructions, parent_developer_instructions) = match (
    multi_agent_version,
    config.multi_agent_v2.subagent_developer_instructions.as_ref(),
) {
    (MultiAgentVersion::V2, Some(_)) => {
        let parent_developer_instructions = match parent_thread
            .session
            .new_default_turn()
            .await
            .developer_instructions
            .clone()
        {
            Some(instructions) if !instructions.is_empty() => Some(instructions),
            _ => None,
        };
        (
            Some(config.developer_instructions.clone().unwrap_or_default()),
            parent_developer_instructions,
        )
    }
    (MultiAgentVersion::Disabled | MultiAgentVersion::V1, _)
    | (MultiAgentVersion::V2, None) => (None, None),
};
```

### 1.3 WorldState 上下文片段系统

WorldState 管理所有运行时上下文片段：

```rust
// codex-rs/core/src/context/world_state/mod.rs
pub struct WorldState {
    // 环境上下文
    pub environment: Option<EnvironmentContext>,
    pub environment_snapshot: Option<TurnEnvironmentSnapshot>,
    
    // 技能和插件
    pub host_skills_catalog: Option<HostSkillsCatalogInWorldState>,
    pub available_skills: Option<AvailableSkillsInstructions>,
    pub available_plugins: Option<AvailablePluginsInstructions>,
    
    // 权限和安全
    pub permissions: Option<PermissionsContext>,
    pub permissions_snapshot: Option<PermissionProfileSnapshot>,
    
    // 多代理和协作
    pub collaboration_mode: Option<CollaborationModeInstructions>,
    pub multi_agent_mode: Option<MultiAgentModeInstructions>,
    
    // 其他上下文
    pub apps: Option<AppsInstructions>,
    pub tools: Option<ToolsInstructions>,
    pub agents_md: Option<AgentsMdInstructions>,
    pub model: Option<ModelContext>,
    pub personality: Option<PersonalitySpecInstructions>,
}
```

### 1.4 上下文更新机制

```rust
// codex-rs/core/src/context_manager/history.rs:406-422
fn trim_pre_turn_context_updates(
    &mut self,
    snapshot: &[ResponseItem],
) {
    // 保存 reference context 用于差分更新
    if let Some((reference_idx, reference_item)) = snapshot
        .iter()
        .enumerate()
        .rfind(|(_, item)| matches!(item, ResponseItem::Message { role, .. } if role == "developer"))
    {
        self.reference_context_item = Some(TurnContextItem::ResponseItem(
            reference_item.clone(),
            reference_idx,
        ));
    } else {
        // 无法差分时清空 baseline，下次需完全重新注入
        self.reference_context_item = None;
    }
}
```

---

## 2. 超长压缩机制

### 2.1 压缩触发条件

#### 2.1.1 Auto Compaction（自动压缩）

```rust
// codex-rs/core/src/config/mod.rs:635-640
pub model_auto_compact_token_limit: Option<i64>,  // 自动压缩阈值
pub model_auto_compact_token_limit_scope: AutoCompactTokenLimitScope,  // 作用域
```

**触发时机**：
```rust
// codex-rs/core/src/session/turn.rs
if let Some(limit) = turn_context.config.model_auto_compact_token_limit {
    if estimated_tokens >= limit {
        // 触发自动压缩
        run_inline_auto_compact_task(sess, step_context, fallback_step_context, initial_context_injection).await?;
    }
}
```

#### 2.1.2 Manual Compaction（手动压缩）

用户通过 `/compact` 命令或 API 调用触发：

```rust
// codex-rs/core/src/compact_token_budget.rs:26-44
pub(crate) async fn run_manual_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
) -> CodexResult<()> {
    let step_context = sess
        .capture_step_context(Arc::clone(&turn_context), &CancellationToken::new())
        .await?;
    let world_state = Arc::new(sess.build_world_state_for_step(&step_context).await);
    run_compact_task_inner(&sess, &step_context, world_state, CompactionTrigger::Manual).await
}
```

### 2.2 压缩实现方式

#### 2.2.1 Inline Compact（本地压缩）

```rust
// codex-rs/core/src/compact.rs:111-188
pub(crate) async fn run_inline_auto_compact_task(
    sess: Arc<Session>,
    turn_context: Arc<TurnContext>,
    initial_context_injection: InitialContextInjection,
    trigger: CompactionTrigger,
) -> CodexResult<()> {
    let compact_prompt = sess
        .config
        .compact_prompt
        .as_deref()
        .unwrap_or(SUMMARIZATION_PROMPT)
        .to_string();
    
    run_compact_task_inner(
        sess,
        turn_context,
        input,
        initial_context_injection,
        compact_prompt,
        trigger,
        CompactionReason::TokenBudget,
        CompactionImplementation::Responses,
        phase,
    ).await
}
```

#### 2.2.2 Remote Compact（远程压缩）

```rust
// codex-rs/core/src/compact_remote.rs:50-73
pub(crate) async fn run_inline_remote_auto_compact_task(
    sess: Arc<Session>,
    step_context: Arc<StepContext>,
    fallback_step_context: Option<Arc<StepContext>>,
    initial_context_injection: InitialContextInjection,
) -> CodexResult<()> {
    let compaction_metadata = CompactionTurnMetadata::new(
        CompactionTrigger::Auto,
        reason,
        CompactionImplementation::ResponsesCompact,
    );
    run_remote_compact_task_inner(
        &sess,
        &step_context,
        fallback_step_context.as_ref(),
        initial_context_injection,
        compaction_metadata,
    ).await
}
```

**远程压缩流程**：
```
本地历史构建 → /responses/compact API → 压缩摘要返回 → 替换历史
```

#### 2.2.3 Responses Compaction V2（新版压缩）

```rust
// codex-rs/core/src/compact_remote_v2.rs:61-85
pub(crate) async fn run_inline_remote_auto_compact_task(
    sess: Arc<Session>,
    step_context: Arc<StepContext>,
    fallback_step_context: Option<Arc<StepContext>>,
    initial_context_injection: InitialContextInjection,
) -> CodexResult<()> {
    let compaction_metadata = CompactionTurnMetadata::new(
        CompactionTrigger::Auto,
        reason,
        CompactionImplementation::ResponsesCompactionV2,
    );
    run_remote_compact_task_inner(
        &sess,
        &step_context,
        fallback_step_context.as_ref(),
        initial_context_injection,
        compaction_metadata,
    ).await
}
```

**V2 改进**：
- 保留更多上下文（`RETAINED_MESSAGE_TOKEN_BUDGET = 64,000`）
- 更好的错误恢复
- 支持 token-budget fallback

### 2.3 初始上下文注入策略

```rust
// codex-rs/core/src/compact.rs:58-74
pub(crate) enum InitialContextInjection {
    BeforeLastUserMessage {
        injected_items: Vec<ResponseItem>,
    },
    DoNotInject,
}
```

**策略选择**：
```rust
// codex-rs/core/src/compact.rs:64-66
// Mid-turn compaction must use BeforeLastUserMessage because the model is trained
// to see the compaction summary as the last item in history after mid-turn compaction
```

### 2.4 上下文窗口适配

```rust
// codex-rs/core/src/compact_remote.rs:365-415
pub(crate) fn trim_function_call_history_to_fit_context_window(
    history: &mut ContextManager,
    turn_context: &TurnContext,
    base_instructions: &BaseInstructions,
) -> (usize, i64) {
    let Some(context_window) = turn_context.model_context_window() else {
        return (0, 0);
    };
    
    let base_tokens = i128::try_from(approx_token_count(&base_instructions.text))
        .unwrap_or(i128::MAX);
    
    let mut estimated_tokens: i128 = base_tokens;
    let mut rewritten_outputs = 0;
    
    for (item, item_tokens) in original_items.iter().zip(item_token_estimates).rev() {
        if i64::try_from(estimated_tokens).unwrap_or(i64::MAX) <= context_window {
            break;
        }
        
        let Some(rewritten_item) = rewritten_output_for_context_window(item) else {
            break;
        };
        
        estimated_tokens = estimated_tokens
            .saturating_sub(item_tokens)
            .saturating_add(approx_token_count(&rewritten_item));
        
        *item = rewritten_item;
        rewritten_outputs += 1;
    }
    
    (rewritten_outputs, estimated_deleted_tokens)
}
```

**输出截断策略**：
```rust
// codex-rs/core/src/compact_remote.rs:416-442
fn rewritten_output_for_context_window(item: &ResponseItem) -> Option<ResponseItem> {
    Some(match item {
        ResponseItem::FunctionCallOutput { id, .. } => {
            ResponseItem::FunctionCallOutput {
                id: id.clone(),
                name: item.name().ok()?,
                content: vec![ContentItem::InputText {
                    text: "[Output truncated to fit context window]".to_string(),
                }],
                call_id: item.call_id().ok()?.clone(),
            }
        }
        // ... 其他类型处理
    })
}
```

---

## 3. Skill 的两层渐进式披露机制

Codex 采用两层渐进式披露模型：
- **Tier 1（技能目录）**：每次轮次显示所有隐式可调用技能的短描述（预算控制）
- **Tier 2（完整指令）**：只为显式提及的技能注入完整 SKILL.md 指令

### 3.1 Tier 1：技能目录注入（每次轮次）

**调用位置**：`build_initial_context_with_world_state`
```rust
// codex-rs/core/src/session/mod.rs:3364-3390
pub(crate) async fn build_initial_context_with_world_state(
    &self,
    turn_context: &TurnContext,
    world_state: &WorldState,
) -> Vec<ResponseItem> {
    if turn_context.config.include_skill_instructions
        && turn_context
            .extension_data
            .get::<HostSkillsCatalogInWorldState>()
            .is_none()
    {
        let available_skills = build_available_skills(
            turn_context.turn_skills.snapshot.outcome(),
            default_skill_metadata_budget(turn_context.model_info.context_window),
            SkillRenderSideEffects::ThreadStart {
                session_telemetry: &self.services.session_telemetry,
            },
        );
        if let Some(available_skills) = available_skills {
            let skills_instructions = AvailableSkillsInstructions::from_available_skills(
                &available_skills,
                turn_context.model_info.include_skills_usage_instructions,
            );
            developer_sections.push(skills_instructions.render());
        }
    }
}
```

**渲染逻辑**：
```rust
// codex-rs/core-skills/src/render.rs:155-196
pub fn build_available_skills(
    outcome: &SkillLoadOutcome,
    budget: SkillMetadataBudget,
    side_effects: SkillRenderSideEffects<'_>,
) -> Option<AvailableSkills> {
    let skills = outcome.allowed_skills_for_implicit_invocation();
    if skills.is_empty() {
        return None;
    }
    
    let absolute_lines = ordered_absolute_skill_lines(&skills);
    let absolute = build_available_skills_from_lines(
        absolute_lines,
        skills.len(),
        budget,
        SkillPathAliases::default(),
    )?;
    
    // ... 渲染技能目录（短描述，预算控制）
}
```

### 3.2 Tier 2：显式技能完整指令注入

**调用位置**：`build_skills_and_plugins`（每次轮次传入用户输入）
```rust
// codex-rs/core/src/session/turn.rs:221-230
let Some((injection_items, explicitly_enabled_connectors)) = build_skills_and_plugins(
    &sess,
    first_step_context.as_ref(),
    &user_input,  // 每次轮次传入用户输入
    &mentioned_plugins,
    &cancellation_token,
).await
```

**注入逻辑**：
```rust
// codex-rs/core/src/session/turn.rs:792-800
let SkillInjections {
    items: skill_injections,
    warnings: skill_warnings,
} = build_skill_injections(
    &mentioned_skills,  // 显式提及的技能
    Some(skills_outcome),
    Some(&turn_context.session_telemetry),
    &sess.services.analytics_events_client,
    tracking.clone(),
).await;
```

**完整指令加载**：
```rust
// codex-rs/core-skills/src/injection.rs:72-120
pub async fn build_skill_injections(
    mentioned_skills: &[SkillMetadata],
    loaded_skills: Option<&SkillLoadOutcome>,
    otel: Option<&SessionTelemetry>,
    analytics_client: &AnalyticsEventsClient,
    tracking: TrackEventsContext,
) -> SkillInjections {
    if mentioned_skills.is_empty() {
        return SkillInjections::default();  // 早期返回：无显式提及
    }
    
    for skill in mentioned_skills {
        match fs.read_file_text(&path, /*sandbox*/ None).await {
            Ok(contents) => {
                result.items.push(SkillInjection {
                    name: skill.name.clone(),
                    path: skill.path_to_skills_md.to_string_lossy().into_owned(),
                    contents,  // 完整的 SKILL.md 内容
                });
            }
            Err(err) => {
                result.warnings.push(format!(
                    "Failed to load skill {name} at {path}: {err:#}",
                    name = skill.name,
                    path = skill.path_to_skills_md.display()
                ));
            }
        }
    }
}
```

### 3.3 显式技能提及收集

**收集逻辑**：
```rust
// codex-rs/core-skills/src/injection.rs:160-203
pub fn collect_explicit_skill_mentions(
    inputs: &[UserInput],
    skills: &[SkillMetadata],
    disabled_paths: &HashSet<AbsolutePathBuf>,
    connector_slug_counts: &HashMap<String, usize>,
) -> Vec<SkillMetadata> {
    // 1. 结构化选择：UserInput::Skill
    for input in inputs {
        if let UserInput::Skill { name, path, .. } = input {
            blocked_plain_names.insert(name.clone());
            // ... 按路径解析技能
        }
    }
    
    // 2. 文本解析：$skill-name tokens
    for input in inputs {
        if let UserInput::Text { text, .. } = input {
            let mentioned_names = extract_tool_mentions(text);
            select_skills_from_mentions(
                &selection_context,
                &blocked_plain_names,
                &mentioned_names,
                &mut seen_names,
                &mut seen_paths,
                &mut selected,
            );
        }
    }
}
```

**提及类型**：
```rust
// codex-rs/core-skills/src/injection.rs:235-253
pub struct ToolMentions<'a> {
    names: HashSet<&'a str>,      // $skill-name tokens
    paths: HashSet<&'a str>,      // skill://path/to/SKILL.md
    plain_names: HashSet<&'a str>, // plain skill names
}

pub enum ToolMentionKind {
    App,
    Mcp,
    Plugin,
    Skill,
    Other,
}
```

### 3.4 Skill 目录预算控制机制

**默认预算计算**：
```rust
// codex-rs/core-skills/src/render.rs:138-153
pub fn default_skill_metadata_budget(context_window: Option<i64>) -> SkillMetadataBudget {
    context_window
        .and_then(|window| usize::try_from(window).ok())
        .filter(|window| *window > 0)
        .map(|window| {
            SkillMetadataBudget::Tokens(
                window
                    .saturating_mul(SKILL_METADATA_CONTEXT_WINDOW_PERCENT)
                    .saturating_div(100)
                    .max(1),
            )
        })
        .unwrap_or(SkillMetadataBudget::Characters(
            DEFAULT_SKILL_METADATA_CHAR_BUDGET,
        ))
}

const DEFAULT_SKILL_METADATA_CHAR_BUDGET: usize = 8_000;
const SKILL_METADATA_CONTEXT_WINDOW_PERCENT: usize = 2;  // 默认 2% 上下文窗口
```

**预算控制逻辑**：
```rust
// codex-rs/core-skills/src/render.rs:319-369
fn render_skill_lines_from_lines(
    skill_lines: Vec<SkillLine<'_>>,
    total_count: usize,
    budget: SkillMetadataBudget,
) -> (Vec<String>, SkillRenderReport) {
    let full_cost = skill_lines.iter().fold(0usize, |used, line| {
        used.saturating_add(line.full_cost(budget))
    });
    
    // 情况 1：预算充足，保留完整描述
    if full_cost <= budget.limit() {
        let included = skill_lines
            .iter()
            .map(SkillLine::render_full)
            .collect::<Vec<_>>();
        return (included, skill_render_report(total_count, skill_lines.len(), 0, 0, 0));
    }
    
    // 情况 2：预算不足，截断描述
    let minimum_cost = skill_lines.iter().fold(0usize, |used, line| {
        used.saturating_add(line.minimum_cost(budget))
    });
    
    if minimum_cost <= budget.limit() {
        let rendered = render_lines_with_description_budget(
            budget,
            &skill_lines,
            budget.limit().saturating_sub(minimum_cost),
        );
        let (truncated_description_chars, truncated_description_count) =
            sum_description_truncation(&rendered);
        let included = rendered
            .into_iter()
            .map(|rendered| rendered.line)
            .collect::<Vec<_>>();
        return (
            included,
            skill_render_report(
                total_count,
                skill_lines.len(),
                0,
                truncated_description_chars,
                truncated_description_count,
            ),
        );
    }
    
    // 情况 3：预算极低，省略部分技能
    render_minimum_skill_lines_until_budget(budget, skill_lines, total_count)
}
```

**警告机制**：
```rust
// codex-rs/core-skills/src/render.rs:200-218
let warning_message = if report.omitted_count > 0 {
    let skill_word = if report.omitted_count == 1 { "skill" } else { "skills" };
    let verb = if report.omitted_count == 1 { "was" } else { "were" };
    Some(format!(
        "{} {} additional {} {} not included in the model-visible skills list.",
        budget_warning_prefix(budget, SKILL_DESCRIPTIONS_REMOVED_WARNING_PREFIX),
        report.omitted_count,
        skill_word,
        verb
    ))
} else if report.average_truncated_description_chars() > SKILL_DESCRIPTION_TRUNCATION_WARNING_THRESHOLD_CHARS {
    Some(match budget {
        SkillMetadataBudget::Tokens(_) => SKILL_DESCRIPTION_TRUNCATED_WARNING_WITH_PERCENT,
        SkillMetadataBudget::Characters(_) => SKILL_DESCRIPTION_TRUNCATED_WARNING,
    }.to_string())
} else {
    None
};
```

### 3.4 Host 技能去重机制

**避免重复注入 host 提供的技能**：
```rust
// codex-rs/core/src/session/turn.rs:848-858
let mut injection_items: Vec<ResponseItem> = match injected_host_skill_prompts {
    Some(injected_host_skill_prompts) => skill_injections
        .iter()
        .filter(|skill| !injected_host_skill_prompts.contains_path(&skill.path))
        .map(|skill| {
            ContextualUserFragment::into(crate::context::SkillInstructions::from(skill))
        })
        .collect(),
    None => skill_items,
};
```

**注意**：这是去重逻辑（避免 host 提供的技能被重复注入），**不是预算控制的卸载机制**。

### 3.5 Skill 生命周期管理

```rust
// codex-rs/core/src/context/world_state/mod.rs
pub struct HostSkillsCatalogInWorldState {
    pub skills: BTreeMap<SkillPath, InjectedSkill>,
    pub version: u64,  // 每次更新递增
}

impl HostSkillsCatalogInWorldState {
    pub fn should_reload(&self, current_version: u64) -> bool {
        self.version != current_version
    }
    
    pub fn update(&mut self, new_skills: Vec<InjectedSkill>) {
        self.skills.clear();
        for skill in new_skills {
            self.skills.insert(skill.path.clone(), skill);
        }
        self.version += 1;
    }
}
```

### 3.6 重要澄清

**Codex 中不存在 Skill 卸载机制**：
- Skill 只在 `build_initial_context` 时注入一次
- 压缩时不会过滤 skill 指令（不同于用户消息）
- 预算控制通过**描述截断**和**技能省略**实现，不是卸载
- `injected_host_skill_prompts` 过滤是**去重**，不是预算卸载

---

## 4. 上下文窗口管理

### 4.1 上下文窗口配置

```rust
// codex-rs/core/src/config/mod.rs:632-633
pub model_context_window: Option<i64>,  // 上下文窗口大小（tokens）
```

### 4.2 Token 估算

```rust
// codex-rs/core/src/context_manager/mod.rs:6-7
pub(crate) use history::estimate_item_token_count;

pub(crate) fn estimate_item_token_count(item: &ResponseItem) -> i64 {
    match item {
        ResponseItem::Message { content, .. } => {
            content.iter()
                .map(|c| estimate_content_token_count(c))
                .sum()
        }
        ResponseItem::FunctionCallOutput { content, .. } => {
            content.iter()
                .map(|c| estimate_content_token_count(c))
                .sum()
        }
        // ... 其他类型
    }
}
```

### 4.3 截断策略

```rust
// codex-rs/core/src/context_manager/history.rs
pub(crate) fn truncate_function_output_payload(
    content: &mut Vec<ContentItem>,
    max_tokens: i64,
) -> i64 {
    let mut current_tokens = 0;
    content.retain_mut(|item| {
        let item_tokens = estimate_content_token_count(item);
        if current_tokens + item_tokens > max_tokens {
            // 截断为占位符
            *item = ContentItem::InputText {
                text: format!("[...{} tokens truncated...]", item_tokens),
            };
            false
        } else {
            current_tokens += item_tokens;
            true
        }
    });
    current_tokens
}
```

---

## 5. 关键源码位置

| 功能 | 文件 | 行号 |
|------|------|------|
| **ContextualUserFragment trait** | `codex-rs/context-fragments/src/fragment.rs` | 22-65 |
| **ContextManager** | `codex-rs/core/src/context_manager/history.rs` | 40-422 |
| **Inline compact** | `codex-rs/core/src/compact.rs` | 111-188 |
| **Remote compact** | `codex-rs/core/src/compact_remote.rs` | 50-73 |
| **Responses compaction V2** | `codex-rs/core/src/compact_remote_v2.rs` | 61-85 |
| **InitialContextInjection** | `codex-rs/core/src/compact.rs` | 17-54 |
| **Context window trimming** | `codex-rs/core/src/compact_remote.rs` | 365-415 |
| **Tier 1: build_initial_context_with_world_state** | `codex-rs/core/src/session/mod.rs` | 3364-3390 |
| **Tier 1: build_available_skills** | `codex-rs/core-skills/src/render.rs` | 155-196 |
| **Tier 2: build_skills_and_plugins** | `codex-rs/core/src/session/turn.rs` | 221-230 |
| **Tier 2: build_skill_injections** | `codex-rs/core-skills/src/injection.rs` | 72-120 |
| **Tier 2: collect_explicit_skill_mentions** | `codex-rs/core-skills/src/injection.rs` | 160-203 |
| **Host skill deduplication** | `codex-rs/core/src/session/turn.rs` | 848-858 |
| **render_skill_lines_from_lines** | `codex-rs/core-skills/src/render.rs` | 319-369 |
| **default_skill_metadata_budget** | `codex-rs/core-skills/src/render.rs` | 138-153 |
| **WorldState** | `codex-rs/core/src/context/world_state/mod.rs` | - |
| **base_instructions injection** | `codex-rs/core/src/client.rs` | 868-885 |

---

## 6. 常量汇总

| 常量 | 值 | 源码位置 |
|------|-----|---------|
| `RETAINED_MESSAGE_TOKEN_BUDGET` | `64_000` | compact_remote_v2.rs:56 |
| `DEFAULT_SKILL_METADATA_CHAR_BUDGET` | `8_000` (chars) | core-skills/src/render.rs:18 |
| `SKILL_METADATA_CONTEXT_WINDOW_PERCENT` | `2` (%) | core-skills/src/render.rs:19 |
| `AUTO_COMPACT_FALLBACK_PROMPT_MAX_BYTES` | `2000` | config/mod.rs:1165 |
| `COMPACT_REQUEST_TIMEOUT_IDLE_MULTIPLIER` | `4` | client.rs:163 |
| `SKILL_DESCRIPTION_TRUNCATION_WARNING_THRESHOLD_CHARS` | `100` | core-skills/src/render.rs:22 |

---

## 7. 总结

Codex 上下文管理系统采用三层架构：

1. **提示词注入层**：
   - `ContextualUserFragment` trait 定义统一接口
   - `base_instructions` 和 `developer_instructions` 提供主提示词
   - WorldState 管理环境、技能、权限等运行时上下文

2. **压缩管理层**：
   - 三种实现：Inline/Remote/ResponsesCompactionV2
   - 自动/手动触发机制
   - `InitialContextInjection` 控制初始上下文注入策略
   - 上下文窗口适配和输出截断

3. **Skill 披露层**（两层渐进式披露，无卸载机制）：
   - **Tier 1（技能目录）**：`build_initial_context_with_world_state` 每次轮次调用 `build_available_skills`，显示所有隐式可调用技能的短描述（预算控制）
   - **Tier 2（完整指令）**：`build_skills_and_plugins` 每次轮次调用 `build_skill_injections`，只为显式提及的技能注入完整 SKILL.md 指令（$skill-name tokens / UserInput::Skill）
   - 三级预算控制：预算充足→保留完整描述；预算不足→截断描述；预算极低→省略技能
   - `injected_host_skill_prompts` 过滤是去重，不是卸载
   - 版本控制的增量更新

核心设计原则：
- **两层渐进式披露**：Tier 1 显示所有技能的短目录（预算控制）；Tier 2 只为显式提及的技能注入完整指令
- **预算控制**：严格限制技能元数据占用（默认 2% 上下文窗口）
- **分级截断**：预算充足→保留完整描述；预算不足→截断描述；预算极低→省略技能
- **差分更新**：基于 `reference_context_item` 的增量更新机制
- **压缩恢复**：多种压缩策略确保长期会话的可持续性
- **无卸载机制**：Skill 通过预算控制（截断/省略）而非动态卸载
