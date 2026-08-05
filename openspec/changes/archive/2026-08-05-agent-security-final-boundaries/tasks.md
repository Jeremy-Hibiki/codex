## 1. 测试先行（RED）

- [x] 1.1 plugin/marketplace 变更 RPC 策略拒绝测试（`plugin_policy`）
- [x] 1.2 settings update danger-full-access 拒绝测试
- [x] 1.3 engaged 时配置变更拒绝测试；未 engaged 放行测试
- [x] 1.4 turn item 输出面红act单元测试 + 集成测试（脚本回显明文）

## 2. 实现（GREEN）

- [x] 2.1 `rpc_guard`：插件/市场与配置变更策略助手；消息分发接线
- [x] 2.2 `thread_settings_update` danger-full-access 拒绝
- [x] 2.3 `redact_turn_item` 扩展输出面红act

## 3. 验证与收尾

- [x] 3.1 app-server 全量 906/906、core 加密相关 96/96
- [x] 3.2 `just fmt` 与 `just fix` 通过
- [x] 3.3 提交并在设计文档/FIX_LOG 记录结论
