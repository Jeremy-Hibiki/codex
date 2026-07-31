# Codex 开发环境配置

## 已安装工具
- **Bazel**: 9.2.0
- **Rust**: 1.95.0 (channel: 1.95.0, components: clippy, rustfmt, rust-src)

> **源码验证**：Rust 版本定义于 `codex-rs/rust-toolchain.toml:2`
- **Just**: 1.57.0
- **Python**: 3.14.6
- **Node**: v24.18.1 / npm: 11.19.0 / pnpm: 10.33.0
- **Git**: 2.55.0
- **cargo-nextest**: 0.9.140
- **cargo-insta**: 1.48.0
- **dotslash**: 0.5.8
- **buildifier**: 8.5.1 (via dotslash cache)

## GitHub 镜像
- URL: https://ghfast.top/
- 配置在 ~/.gitconfig: [url "https://ghfast.top/github.com/"] insteadOf = https://github.com/

## Git 代理
- 配置在 ~/.gitconfig-github: proxy = http://192.168.126.193:7890

## V8 预编译二进制缓存
- 路径: ~/.cargo/.rusty_v8/
- 文件: librusty_v8_release_x86_64-unknown-linux-gnu.a.gz (通过 ghfast.top 镜像下载)

## CARGO_HOME
- /home/vscode/.cargo

## 验证状态
- [✓] 全工作区 cargo check --workspace
- [✓] just fmt (格式化)
- [✓] just clippy (代码检查)
- [✓] just test (测试框架)
- [✓] pnpm install (JS 依赖)
