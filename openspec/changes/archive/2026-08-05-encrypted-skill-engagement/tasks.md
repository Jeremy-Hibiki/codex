## 1. 测试先行（RED）

- [x] 1.1 写失败单测：`is_engaged` 空会话 false、加载成功后 true、`clear_thread` 后 false
- [x] 1.2 写失败单测：解密 in-flight 期间 `is_engaged` 为 true（用阻塞 SDK 制造“解密中未登记”窗口）
- [x] 1.3 写失败单测：解密失败后 in-flight 移除、部分写入目录被 `secure_wipe`、`is_engaged` 回到 false
- [x] 1.4 写失败单测：`path_mappings` 返回 `(dir, original_dir)`、空 original_dir 排除、无 skill 返回空列表

## 2. 实现（GREEN）

- [x] 2.1 runtime 增加 `in_flight: Mutex<HashSet<String>>` 与 RAII `InFlightGuard`（Drop 移除）
- [x] 2.2 `load_or_register` 解密路径插入 in-flight；成功/失败均由 guard 移除，失败继续 `secure_wipe`
- [x] 2.3 实现 `is_engaged(session_id)`（registry 非空 || in_flight 含 session）
- [x] 2.4 实现 `path_mappings(session_id)`（仅导出非空 original_dir）
- [x] 2.5 `clear_thread` 同时清理该 session 的 in-flight 标记

## 3. 验证与收尾

- [x] 3.1 `just test -p fm-encrypted-skills` 全绿（含既有回归）
- [x] 3.2 `just fmt` 与 `just fix -p fm-encrypted-skills` 通过
- [x] 3.3 提交变更并在 `FM_AGENT_SECURITY_FIX_LOG.md` 记录决策
