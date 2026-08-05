# Codex Bazel 构建指南（国内网络环境）

本文档描述如何在受限网络环境（无法直接访问 GitHub / crates.io / Google Source）下，使用 Bazel 构建带 FMSH License 的 Codex CLI 二进制。

## 前置要求

- Linux x86_64（glibc ≥ 2.28，推荐 Ubuntu 20.04+）
- Python 3（用于 BCR registry proxy）
- 网络：需能访问 `ghfast.top`（GitHub 反向代理）、`rsproxy.cn`（crates.io 镜像）
- 内网 License Server：`192.168.131.126:8089`（lmclient-rust-sdk git 依赖）

## 构建方式

### 方式一：本地 Bazel 构建

```bash
# 1. 启动 BCR registry proxy（重写 BCR JSON 中的 github.com → ghfast.top）
nohup python3 release/bazel-registry-proxy.py 8765 > /tmp/proxy.log 2>&1 &

# 2. 预下载无法 patch 的 BCR 内部归档到 distdir
bash release/predownload-deps.sh

# 3. 构建
bazel build //codex-rs/cli:codex

# 4. 验证
bazel-bin/codex-rs/cli/codex --version
# 输出: codex 0.146.0-fm.1
```

### 方式二：Docker 构建（推荐，用于分发）

```bash
DOCKER_BUILDKIT=1 docker build \
  --build-arg HTTP_PROXY="$HTTP_PROXY" \
  --build-arg HTTPS_PROXY="$HTTPS_PROXY" \
  --build-arg NO_PROXY="$NO_PROXY" \
  --build-arg UBUNTU_VERSION=20.04 \
  -t codex:ubuntu-20.04 \
  -f release/Dockerfile .

# 提取二进制
id=$(docker create codex:ubuntu-20.04)
docker cp "$id:/usr/local/bin/codex" ./codex
docker rm "$id"
```

Docker 构建支持 `UBUNTU_VERSION` ARG 切换基础镜像版本（20.04 / 22.04）。
BuildKit cache mount 会自动持久化 Bazel 编译缓存，增量构建跳过已编译的 action（V8 等）。

## 网络代理架构

所有对 `github.com` 的访问通过三层机制代理到 `ghfast.top`：

| 下载类型 | 机制 | 涉及文件 |
|---|---|---|
| BCR 模块元数据 (JSON) | 本地 HTTP proxy 重写 URL | `release/bazel-registry-proxy.py` + `.bazelrc --registry` |
| `http_archive` / `http_file` | MODULE.bazel / patch 中直接写 ghfast.top URL | `MODULE.bazel`, `patches/v8_module_deps.patch`, `patches/rules_rs_rust_archive_url.patch` |
| `git_repository` | MODULE.bazel 中使用镜像 URL；Docker 中用 `git config url.insteadOf` | `MODULE.bazel` (Jeremy-Hibiki 镜像), `release/Dockerfile` |
| BCR 模块内部下载（无法 patch） | 预下载到 distdir，Bazel 按 sha256 匹配 | `release/predownload-deps.sh` |
| crates.io | rsproxy.cn 镜像 | `~/.cargo/config.toml` |
| Bazel 本体 | bazelisk + `BAZELISK_BASE_URL` (Docker) | `release/Dockerfile` |

### 非 github.com 的镜像

以下仓库原始 URL 不是 github.com，用 GitHub 上的镜像替代：

| 原始 URL | 镜像 URL |
|---|---|
| `chromium.googlesource.com/.../llvm-project/libcxx.git` | `ghfast.top/github.com/Jeremy-Hibiki/libcxx.git` |
| `chromium.googlesource.com/.../llvm-project/libcxxabi.git` | `ghfast.top/github.com/Jeremy-Hibiki/libcxxabi.git` |
| `chromium.googlesource.com/.../llvm-project/libc.git` | `ghfast.top/github.com/Jeremy-Hibiki/libc.git` |
| `chromium.googlesource.com/chromium/deps/icu.git` | `ghfast.top/github.com/Jeremy-Hibiki/icu.git` |
| `static.crates.io/crates/v8/` | `rsproxy.cn/api/v1/crates/v8/` |

## License 集成

### 环境变量

| 环境变量 | 来源 | 必填 | 默认值 | 说明 |
|---|---|---|---|---|
| `FMSH_LIC_SERVER` | LMCLIENT SDK 内部 | ✅ | — | LicenseService 地址 `<port>@<host>` |
| `FMSH_CODEX_LIC_FEATURE` | fm-license crate | ✅ | — | 特性名 |
| `FMSH_CODEX_LIC_VERSION` | fm-license crate | ✅ | — | 特性版本 |
| `FMSH_CODEX_LIC_DISPLAY_NAME` | fm-license crate | ❌ | `"Codex"` | 显示名 |

### 拦截行为

- **所有构建模式（debug + release）无条件强制验证**
- 无任何环境变量逃逸口
- 拦截范围：TUI interactive (`None`)、`exec`、`review` 三个产品子命令
- 不拦截：`--version`、`--help`、`app-server`、`mcp`、`auth` 等元命令/辅助子命令
- lmclient SDK 仅在 `target_env = "gnu"` 下编译（Linux x86_64 glibc）

### lmclient 静态库链接

lmclient-rust-sdk 在源码树内打包了 `liblmclient.a`（CentOS 7 glibc 编译）。由于 rules_rust 的 sandbox 机制，build.rs 的 `cargo:rustc-link-search` 输出对 rustc 编译 action 不可见，因此用 `cc_import` + `cc_library` 替代：

- `third_party/lmclient/additive.BUILD.bazel` — 定义 cc_import 和系统库链接
- MODULE.bazel 的 `crate.annotation(gen_build_script = "off")` — 禁用 build.rs

## 版本号注入

Bazel 的 rules_rust 默认将 `CARGO_PKG_VERSION` 设为 `0.0.0`（不像 Cargo 那样读 Cargo.toml）。
`defs.bzl` 中的 `WORKSPACE_VERSION` 常量通过 `rust_library` 和 `rust_binary` 的 `version` 属性注入正确版本号，使 `env!("CARGO_PKG_VERSION")` 在编译期正确解析。

## distdir 预缓存

`release/bazel-distdir/` 存放预下载的依赖归档。Bazel 的 `--distdir` 按 **sha256** 匹配文件（不看 URL），所以即使 MODULE.bazel 里的 URL 被改写过，distdir 里的原始归档也能命中。

`release/predownload-deps.sh` 负责下载那些 **BCR 模块内部** 引用的、无法通过 MODULE.bazel patch 修改的 github.com 归档（如 `bsdtar-prebuilt`、`bats-core`）。如果新版本出现新的未覆盖 URL，在此脚本中添加即可。

## 产出规格

```
$ codex --version
codex 0.146.0-fm.1

$ file codex
ELF 64-bit LSB pie executable, x86-64, dynamically linked, interpreter /lib64/ld-linux-x86-64.so.2

$ readelf -V codex | grep -oP 'GLIBC_\K[0-9.]+' | sort -V | tail -1
2.28
```

- 动态链接，max GLIBC 2.28（兼容 Ubuntu 20.04+ / glibc 2.31+）
- 依赖系统库：`libcurl4`、`libstdc++6`、`libzstd1`（Ubuntu 20.04+ 自带）
- OpenSSL / AWS-LC / V8 等由 hermetic LLVM 工具链静态链接

## 涉及文件清单

```
.bazelrc                                    Bazel 配置（registry proxy, distdir, jobs）
BUILD.bazel                                 顶层平台定义（含 musl release platforms）
MODULE.bazel                                Bazel 模块声明（ghfast URLs, Jeremy-Hibiki 镜像, lmclient 注入）
defs.bzl                                    Rust crate 宏（WORKSPACE_VERSION 注入）
patches/v8_module_deps.patch                V8 模块依赖重构 + ghfast URLs
patches/rules_rs_rust_archive_url.patch     rules_rs 内部 URL → ghfast.top
third_party/lmclient/additive.BUILD.bazel   lmclient cc_import + 系统库链接
release/Dockerfile                          多阶段 Docker 构建（Ubuntu 20.04, BuildKit cache）
release/bazel-registry-proxy.py             BCR GitHub URL 重写代理
release/predownload-deps.sh                 BCR 内部归档预下载脚本
release/bazel-distdir/                      预下载的依赖归档（按 sha256 匹配）
.dockerignore                               Docker 构建上下文排除
```

## 迁移到其他分支

将以下文件复制到目标分支，然后根据该分支的 V8 版本调整 URL：

1. **基建文件**（直接复制）：
   `.bazelrc`、`release/`（Dockerfile, proxy, predownload, distdir）、`.dockerignore`

2. **需要手动合并**（目标分支的 MODULE.bazel 和 patches 有版本差异）：
   - `MODULE.bazel`：添加 rules_rs patch、ghfast URLs、Jeremy-Hibiki 镜像、rsproxy URL
   - `patches/v8_module_deps.patch`：在该分支原始 patch 基础上改 URL（不要整体覆盖）
   - `patches/rules_rs_rust_archive_url.patch`：直接复制
   - `defs.bzl`：添加 `WORKSPACE_VERSION` 常量和 `version` 属性注入

3. **License 专属**（仅 fm-license 分支需要）：
   - `codex-rs/fm/license/` crate
   - `third_party/lmclient/` additive build
   - MODULE.bazel 的 lmclient `crate.annotation`
