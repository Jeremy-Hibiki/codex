## 1. 测试先行（RED）

- [x] 1.1 core：`ensure_encrypted_skill_sandbox`（engaged+无沙箱拒绝、engaged+有沙箱放行、未 engaged+无沙箱放行）
- [x] 1.2 cli：full-access 参数与 plugin 子命令拒绝（更新/新增 cli 测试）

## 2. 实现（GREEN）

- [x] 2.1 core helper + orchestrator 接线
- [x] 2.2 cli_main：拒绝 danger-full-access/bypass 与 plugin 子命令
- [x] 2.3 app-server：`thread/start`、`turn/start` danger-full-access 拒绝
- [ ] 2.4 app-server 插件/marketplace RPC 拒绝（最终阶段接线，标记）

## 3. 验证与收尾

- [x] 3.1 相关测试全绿（core、cli、app-server）
- [x] 3.2 `just fmt` 与 `just fix` 通过
- [x] 3.3 提交变更并在 `FM_AGENT_SECURITY_FIX_LOG.md` 记录决策
