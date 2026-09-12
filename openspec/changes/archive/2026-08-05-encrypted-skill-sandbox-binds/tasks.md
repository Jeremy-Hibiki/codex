## 1. 测试先行（RED）

- [x] 1.1 protocol：`ReadonlyBind`/`readonly_binds` serde round-trip 与缺省空列表测试
- [x] 1.2 linux-sandbox：binds 生成 `--ro-bind`、缺失 target `--dir`、缺失 source 跳过、空 binds 不变
- [x] 1.3 core：`sandbox_applies_binds` 判定（bwrap 生效/跳过/legacy landlock）
- [x] 1.4 core：guard 逻辑路径 execute-only 放行、逻辑路径读拦截、无 binds 时保持旧重写
- [x] 1.5 core：engaged 技能脚本自动 Permit 判定（含非技能命令仍需审批）

## 2. 实现（GREEN）

- [x] 2.1 protocol 新增 `ReadonlyBind` 与 `readonly_binds`
- [x] 2.2 bwrap 应用 binds（顺序、`--dir`、跳过缺失 source）
- [x] 2.3 core `sandbox_applies_binds` + `SandboxAttempt.skill_binds` + `env_for`/`env_for_exec_server` 注入
- [x] 2.4 guard 重写模式（binds 生效保留逻辑路径；否则旧重写）与逻辑路径受保护
- [x] 2.5 shell/unified exec 审批起点 D9 自动 Permit

## 3. 验证与收尾

- [x] 3.1 相关 crate 测试全绿（protocol 265/265、core guard 71/71、linux-sandbox bwrap 55/55；两例网络类集成测试为环境性失败）
- [x] 3.2 `just fmt` 与 `just fix`（涉及 crate）通过
- [x] 3.3 提交变更并在 `FM_AGENT_SECURITY_FIX_LOG.md` 记录决策
