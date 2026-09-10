# fmsh-ukey-lib 接入分析（Skill 加解密）

- 上游：`http://192.168.131.126:8089/jiangzhengqi/fmsh-ukey-lib.git`
- 版本：master `6bd55f8691f0f60ec5f7b9b7f804a5e4899023b1`（无 tag）
- 日期：2026-08-05

## 1. 结论摘要

上游提供的目标行为与我们的 `EnvelopeSdk` 契约高度吻合：`.zip.enc`
是“**整个 skill 目录 zip 之后做一层数字信封**”，信封算法分
`CMS-SM2-SM4`（UKey 硬件）、`X25519-AES-256-GCM`（本地密钥）与 `MOCK`
（仅测试）。Codex 运行期只需要**解密**：读 `.zip.enc` → 用对应 cipher
解出 zip → 按现有规则解压/校验/落盘。

直接阻塞：`fmsh-ukey-wrapper/build.rs` **强制要求构建期
`FMSH_UKEY_SDK_DIR`** 指向含 `linux/lib/libfmsh_ukey_sdk.so` 的 SDK 目录，
没有 fallback；本环境未设置该变量，也没有 SDK 文件。因此任何直接依赖
`fmsh-ukey-cipher`（其无条件依赖 wrapper）的代码，在当前环境无法编译，
必须先解决 SDK 供给或让上游支持“无 SDK 编译（运行时才报错）”。

## 2. 上游仓库结构

| crate | 职责 |
|-------|------|
| `crates/fmsh-ukey-wrapper` | 供应商 SDK 的安全封装：进程级 init（OnceLock+Mutex，失败不缓存）、`enum_devices`、`export_encryption_cert`、`encrypt(cert, data)`（离线，不需要硬件）、`decrypt(envelope, device, container)`（需 UKey，PIN 自动派生）、`finalize` |
| `crates/fmsh-ukey-enc` | Skill 打包 CLI：`encrypt-skill` / `decrypt-skill` / `batch-encrypt-skills`，后端 mock / local / sdk |
| `crates/fmsh-ukey-cipher` | 可插拔文件加解密库+CLI：`Cipher` trait + `noop`/`local`/`ukey`/`ukey-two-phase` 四后端 |
| `packages/*` | Python/Node/Bun/C++ 绑定（与 Rust 同 API，本地信封格式互操作） |

环境变量：

| 变量 | 用途 | 时机 |
|------|------|------|
| `FMSH_UKEY_SDK_DIR` | SDK 根目录（`linux/lib/libfmsh_ukey_sdk.so`） | **构建期必需**，wrapper build.rs 硬性要求 |
| `FMSH_UKEY_PROVIDER` | 运行时 provider 动态库路径（如 `libgm3000.1.0.so`） | 初始化必需（Python 绑定可内置 vendor，Rust 无 fallback） |
| `FMSH_UKEY_CONTAINER` | UKey 容器名 | 导出证书/解密必需（无内置默认） |

## 3. fmsh-ukey-cipher 的加解密行为

`Cipher: Send + Sync`：

```rust
fn mode(&self) -> &'static str;
fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>>;
fn decrypt(&self, ciphertext: &[u8]) -> Result<Vec<u8>>;
```

后端：

| mode | 算法/格式 | 硬件 |
|------|-----------|------|
| `noop` | 恒等变换 | 无 |
| `local` | 每条消息临时 X25519 + ECDH + HKDF-SHA256 → AES-256-GCM；信封 `[eph_pub 32][iv 12][tag 16][ciphertext]` | 无（解密用静态私钥 PEM） |
| `ukey` | CMS SM2/SM4 信封（`FM_EncryptEnvelopeBuffer` / `FM_DecryptEnvelopeBuffer`） | 仅解密需要 UKey |
| `ukey-two-phase` | 阶段1 UKey 解开一个 32 字节 AES-256-GCM 密钥信封；阶段2 所有文件软件解密；数据文件 `[magic "FMSH2PH1" 8][iv 12][tag 16][ciphertext]` | 一次 UKey 调用 |

`KeyWrap` trait 让 two-phase 逻辑可在无硬件时用软件 wrap 测试。

## 4. Skill 打包格式（fmsh-ukey-enc 定义）

`encrypt-skill` 产物（`encrypted/<skill>/`）：

```
SKILL.md          # 桩：保留 frontmatter + metadata.encrypted:true
                  #      + metadata.encryption:{version:1,key_id,algorithm,package}
<name>.zip.enc    # 整个 skill 目录的 deflate zip 再套一层信封
references/...    # 空桩文件（保留目录结构）
scripts/...
```

- zip 条目权限 `0o600`，路径使用 `/` 且保留相对结构；
- `algorithm` 取值：`CMS-SM2-SM4`（sdk）、`X25519-AES-256-GCM`（local）、
  `MOCK`（mock）；
- `decrypt-skill` 自行解包到 `/dev/shm/skill-<rand>/`（上游代码未显式 chmod
  0700，README 声称 0700；我们运行时不应复用其目录逻辑，继续用我们自己的
  0700 命名空间布局）；
- `batch-encrypt-skills` 递归发现含 `SKILL.md` 的目录，跳过已有 `.enc`。

## 5. 与现有 `EnvelopeSdk` 的映射

当前契约（`codex-rs/fm/encrypted-skills/src/sdk.rs`）：

```rust
trait EnvelopeSdk: Send + Sync {
    fn decrypt_package(&self, package_path: &Path) -> Result<Vec<PackageEntry>, EnvelopeError>;
}
```

`TestZipSdk` 目前把 `.zip.enc` 直接当 zip 解（模拟加密占位）。真实接入后：

1. 读 `<name>.zip.enc`（信封）→ 容量预检（信封大小，防止超限）；
2. `cipher.decrypt(envelope)` → zip 字节（UKey 模式为一次硬件调用；
   local 模式为软件 ECDH+GCM）；
3. 按现有 `MAX_PACKAGE_ENTRIES` / `MAX_PACKAGE_BYTES` 解压校验；
4. 返回 `PackageEntry` 列表，交给现有 `write_package_entries`（0700、
   zip-slip 校验、secure wipe 生命周期全部复用）。

错误映射：上游 `anyhow::Result` → `EnvelopeError::Decrypt(...)`；
SDK 未配置/未初始化 → 复用现有 `HardwareKeyRequired` / `SdkUnavailable`。

## 6. 建议接入方案

### 6.1 依赖与构建门控

- 在 `fm-encrypted-skills` 增加 optional 依赖
  `fmsh-ukey-cipher = { git = "http://192.168.131.126:8089/jiangzhengqi/fmsh-ukey-lib.git", rev = "6bd55f8..." }`
  （建议上游打 tag `v0.3.0` 后改 `tag`）；
- `[features] fmsh-ukey = ["dep:fmsh-ukey-cipher"]`，默认关闭，普通 CI
  不编译 SDK 链；部署镜像带 SDK 时启用；
- `SdkKind` 增加 `UKey` / `Local`；feature 关闭时 `sdk_for` 一律返回
  `UnavailableSdk` 并 `tracing::warn!`（fail-closed，与现状一致）；
- 平台门控：UKey 仅 `cfg(all(target_os="linux", target_arch="x86_64", target_env="gnu"))`
  （供应商 SDK 是 CentOS 7 x86_64），其他平台只保留 local（若允许）。

### 6.2 SDK 实现

- `UKeySdk`：持 `Arc<UkeyCipher>`（cert 传 `None`，device/container 默认
  环境解析）；`decrypt_package` = 读文件 → `cipher.decrypt` → 解 zip →
  校验；
- `LocalSdk`：`LocalCipher::from_priv_file(config 路径)`，无硬件；
- 可选优化：如果后续切换为 two-phase 打包（key envelope + 每文件 .enc），
  再引入 `UkeyTwoPhaseCipher` + `UkeyKeyWrap`；当前单信封模型一次 skill 加载
  就是一次 UKey 调用，多 skill 时硬件调用串行，符合上游 SDK 序列化约束。

### 6.3 配置

`EncryptedSkillsSdkToml` 增加 `ukey` / `local`：

```toml
[encrypted_skills]
sdk = "ukey"            # 或 "local"
# local 模式需要私钥路径，例如：
# local_privkey = "/path/to/enc.priv.pem"
```

UKey 的 provider/container 走环境变量 `FMSH_UKEY_PROVIDER` /
`FMSH_UKEY_CONTAINER`，不落配置。

### 6.4 测试策略

- 无硬件可测：`local` 模式全链路（生成密钥 → 用上游 CLI/库加密 zip →
  我们 `LocalSdk` 解密 → 现有 suite 跑通）；
- UKey 模式：需要真实 GM3000 + provider `.so`，做冒烟/集成测试
  （`FMSH_UKEY_PROVIDER` + `FMSH_UKEY_CONTAINER`），CI 中作为可选 job；
- 包兼容性：用上游 `fmsh-ukey-enc encrypt-skill --mode sdk/local` 产出的
  目录直接喂给我们的 loader，验证 frontmatter 元数据（`encryption.package`
  已支持）与信封解包。

## 7. 阻塞点与决策项

1. **SDK 供给**：构建 `fmsh-ukey-cipher` 必须能拿到
   `FMSH_UKEY_SDK_DIR/linux/lib/libfmsh_ukey_sdk.so`。需要决定：部署/CI 镜像
   提供 SDK 目录；或让上游 wrapper 支持“无 SDK 时以 stub 编译、运行时报错”
   （推荐，便于我们无硬件环境下编译并测 local 模式）。
2. **依赖 pin**：上游目前无 tag，建议打 `v0.3.0` 后按 tag 依赖。
3. **本地密钥路径配置字段名**（`local_privkey` 等）与是否暴露 UKey
   device/container 的配置覆盖（当前计划只用环境变量）。
4. **是否采用 two-phase 打包**：多 skill 单 turn 场景若 UKey 延迟敏感，
   后续可加 key-envelope 变体；当前单信封已可工作。
5. **加密侧**：运行期只解密；加密由上游 CLI/绑定离线完成（需导出证书或
   本地密钥对），不接入 Codex 进程。

## 8. 实施状态

- 已完成：
  1. README/源码理解与接入方案（本文档）；
  2. 配置面骨架（`SdkKind::UKey`/`Local` + `sdk = "ukey"/"local"`，
     fail-closed，commit `5d9fae664c`）；
  3. 上游 `f09dc46` 把 openssl 下限提到 `0.10.76`（`kdf::hkdf` 在该版本
     恢复）；`fmsh-ukey-cipher` 以 optional git 依赖（rev 锁定
     `f09dc46…`）引入（commit `41c2632016`），openssl 升到 `0.10.81`；
  4. `UKeySdk`/`LocalSdk` 实现与 `local_privkey` 配置（commit `33f94b4371`），
     feature 构建 + local 信封回环测试通过（stub SDK 验证，`136 passed`）。
- 待办：
  1. 真实 UKey 硬件冒烟测试（需要 `FMSH_UKEY_PROVIDER` +
     `FMSH_UKEY_CONTAINER` + GM3000）；
  2. 上游打 tag 后把 git 依赖的 rev 换成 tag；
  3. 若采用 two-phase 打包（key envelope + 每文件 .enc），再接入
     `UkeyTwoPhaseCipher` + `UkeyKeyWrap`（当前单信封模型是每次 skill
     加载一次 UKey 调用）。

## 9. 2026-08-06 更新（供说明，覆盖上文过期表述）

- **模式命名**：上游 `local` 已更名 `software`；当前四模式为 `software`
  （HPKE `hpke-x25519-aes256-gcm` 默认 / 标准 CMS `sm2-sm4-cbc`）、`ukey`、
  `ukey-two-phase`、`test_zip`/`noop`（测试）。`algorithm` 值相应为
  `hpke-x25519-aes256-gcm` / `sm2-sm4-cbc` / `AES-256-GCM`（two-phase）。
- **two-phase 已落地**：上游 `e09d40a` 起每个 skill 包自带 `key.enc`
  （CMS 包装的 32 字节 AES-256-GCM 密钥），`--key-envelope` 改名
  `--key-envelope-backup-to`（可选备份）；解密默认读包内 `key.enc`。
  Codex 侧 `UkeyTwoPhaseSdk` 已实现：按 `key.enc` 内容寻址缓存，同一把
  key 只调一次 UKey，后续包全部软件 AES-256-GCM 解密。
- **依赖同步**：`fmsh-ukey-cipher` 已 pin 到 `e7986f1b`（2026-08-06）；
  库 API 未变，Codex 侧无需改调用。
- **真实样例验证**：`encrypt-skill-sample/` 下 software-hpke /
  software-sm2sm4 / ukey-two-phase 三套真实 CLI 产物均可用 Codex
  `EnvelopeSdk` 解密（ukey / ukey-two-phase 硬件路径需 UKey 环境，
  当前沙箱无硬件，未跑实机）。
- **上文 §8 待办 3 已关闭**：two-phase 不再属于待办；待办收敛为“真实 UKey
  硬件冒烟测试”与“上游打 tag 后换 tag 依赖”。
