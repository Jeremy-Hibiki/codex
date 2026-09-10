## 1. 测试先行（RED）

- [x] 1.1 fm：进程级注册表 `any_engaged`/`engaged_guarded_paths`（无 engaged 空、单 runtime engaged、Drop 后失效）
- [x] 1.2 core：telemetry 预览 engaged 时红act、未 engaged 时原样（构造工具结果断言预览）
- [x] 1.3 core：RPC 纯函数——fs 路径命中、命令引用受保护路径、注入/元数据参数命中

## 2. 实现（GREEN）

- [x] 2.1 fm runtime 注册表（Weak）+ `any_engaged` + `engaged_guarded_paths`
- [x] 2.2 core registry：工具结果先包 `RedactingToolOutput` 再取 `log_preview`
- [x] 2.3 core `agent_security::rpc` 纯函数（fs path/command/args）
- [x] 2.4 app-server 处理器接线：fs 读/枚举/watch、fs 写/复制/删除、`command/exec`、`thread/shellCommand`、`process/spawn`、`thread/inject_items`、`thread/name|goal|metadata`

## 3. 验证与收尾

- [x] 3.1 部分完成（fm/core 全绿；app-server RPC E2E 集成测试留待最终验证阶段） 相关 crate 测试全绿（fm、codex-core、app-server 集成）
- [x] 3.2 `just fmt` 与 `just fix`（涉及 crate）通过
- [x] 3.3 提交变更并在 `FM_AGENT_SECURITY_FIX_LOG.md` 记录决策
