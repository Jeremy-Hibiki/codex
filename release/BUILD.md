# Grevo Cargo 构建指南（国内网络环境）

本文档描述如何在受限网络环境下，使用 Cargo 构建带 FMSH License 的 Grevo CLI。发布产物直接由 Cargo release 构建生成，不再使用 Bazel、Docker 或构建脚本。

## 前置要求

- Linux x86_64，Ubuntu 22.04 或更高版本。
- Rust `1.95.0`，由 `codex-rs/rust-toolchain.toml` 固定。
- C/C++ 构建工具：`gcc-10`、`g++-10`、`make`、`perl`、`cmake`、`pkg-config`。
- 网络可访问 `rsproxy.cn`、`ghfast.top` 和内网 License Server。
- 内网 git 依赖服务器：`192.168.131.126:8089`。

Ubuntu 22.04 可使用以下命令安装构建依赖：

```bash
sudo apt-get update
sudo apt-get install -y \
  build-essential ca-certificates cmake curl git gcc-10 g++-10 \
  make perl pkg-config xz-utils

sudo update-alternatives --install /usr/bin/cc cc /usr/bin/gcc-10 100
sudo update-alternatives --install /usr/bin/c++ c++ /usr/bin/g++-10 100
sudo update-alternatives --install /usr/bin/gcc gcc /usr/bin/gcc-10 100
sudo update-alternatives --install /usr/bin/g++ g++ /usr/bin/g++-10 100
```

## Cargo 网络配置

建议在 `~/.cargo/config.toml` 使用 rsproxy.cn 作为 crates.io 镜像，并让 Cargo 使用系统 git：

```toml
[source.crates-io]
replace-with = "rsproxy-sparse"

[source.rsproxy]
registry = "https://rsproxy.cn/crates.io-index"

[source.rsproxy-sparse]
registry = "sparse+https://rsproxy.cn/index/"

[net]
git-fetch-with-cli = true
```

GitHub git 依赖可通过 `ghfast.top` 代理：

```bash
git config --global \
  url."https://ghfast.top/github.com/".insteadOf \
  "https://github.com/"
```

构建时设置 V8 下载镜像：

```bash
export CARGO_NET_GIT_FETCH_WITH_CLI=true
export RUSTY_V8_MIRROR=https://ghfast.top/github.com/denoland/rusty_v8/releases/download
```

## 构建命令

发布构建默认使用静态 SDK 链接模式。SDK 归档和 vendored libcrypto 会嵌入二进制，`libstdc++` 保持动态链接，避免与 V8 的 C++ ABI 冲突。

```bash
cd codex-rs

export FMSH_UKEY_SDK_LINK=static
export FMSH_UKEY_LIBSTDCPP=shared

cargo build --release --locked -p codex-cli --bin grevo
```

注意：

- Cargo 包名仍然是 `codex-cli`。
- 二进制目标名是 `grevo`。
- 产物路径是 `codex-rs/target/release/grevo`。

如果需要兼容旧的动态 SDK 模式：

```bash
cd codex-rs

export FMSH_UKEY_SDK_LINK=shared
unset FMSH_UKEY_LIBSTDCPP

cargo build --release --locked -p codex-cli --bin grevo
```

动态模式需要随产物分发 `libfmsh_ukey_sdk.so.0`，并保证系统提供 `libcrypto.so.3`。推荐使用前面的静态模式。

## 版本号

`codex-rs/cli/build.rs` 会根据最近的 `rust-v*` tag 自动生成版本后缀：

```text
0.146.0-fm.rNNN-HHHHHHHH
```

- `NNN`：距最近 `rust-v*` tag 的提交数。
- `HHHHHHHH`：短提交哈希。

正常输出：

```text
$ codex-rs/target/release/grevo --version
grevo 0.146.0-fm.rNNN-HHHHHHHH
```

如果构建环境没有 git 元数据，或者需要固定版本后缀，显式设置 `FM_BUILD_SUFFIX`：

```bash
cd codex-rs

FM_BUILD_SUFFIX=fm.r37-456e4457 \
FMSH_UKEY_SDK_LINK=static \
FMSH_UKEY_LIBSTDCPP=shared \
cargo build --release --locked -p codex-cli --bin grevo
```

## 运行时依赖

静态 SDK 模式只依赖系统 `libstdc++`。UKey 的 GM3000 provider 仍然通过 `dlopen` 加载，需要把 `libgm3000.1.0.so` 与二进制一起分发，并设置：

```bash
export FMSH_UKEY_PROVIDER=/path/to/lib/libgm3000.1.0.so
```

也可以在二进制同级的 `lib/` 目录放置 provider，二进制的 RUNPATH 已包含 `$ORIGIN/lib`。

License 运行时需要配置：

| 环境变量 | 必填 | 说明 |
|---|---|---|
| `FMSH_LIC_SERVER` | 是 | License Server 地址，格式 `<port>@<host>` |
| `FMSH_CODEX_LIC_FEATURE` | 是 | License 特性名 |
| `FMSH_CODEX_LIC_VERSION` | 是 | License 特性版本 |
| `FMSH_CODEX_LIC_DISPLAY_NAME` | 否 | 显示名，默认 `Grevo` |
| `FMSH_CODEX_LIC_HOSTNAME` | 否 | 上报给 License Server 的主机名 |

## 产物校验

```bash
cd codex-rs

file target/release/grevo
ldd target/release/grevo
readelf -d target/release/grevo | grep -E 'NEEDED|RUNPATH'
target/release/grevo --version
```

静态模式下，`ldd` 不应出现 `libfmsh_ukey_sdk.so.0` 或 `libcrypto.so.3`。动态模式下，这两个库需要由产物目录或系统提供。

## 发布检查

1. 使用 `--release --locked` 构建。
2. 确认 `grevo --version` 的版本后缀与本次提交一致。
3. 确认静态模式下没有意外的 SDK 动态依赖。
4. 分发 `grevo` 以及运行所需的 `libgm3000.1.0.so`。
5. 在目标机器上使用真实 License Server 完成启动验证。
