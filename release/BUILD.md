# Codex Bazel 构建指南（国内网络环境）

本文档描述如何在受限网络环境（无法直接访问 GitHub / crates.io / Google Source）下，使用 Bazel 构建带 FMSH License 的 Codex CLI 二进制。

## 前置要求

- Linux x86_64（Cargo 发布构建默认静态链入 fmsh-ukey SDK 与 vendored
  libcrypto，glibc 上限 < 2.33，支持 Ubuntu 20.04+；`--sdk-link shared`
  退回动态 SDK `.so`，则需 glibc ≥ 2.34 / Ubuntu 22.04+ 与宿主 libssl3）
- Python 3（用于 BCR registry proxy）
- 网络：需能访问 `ghfast.top`（GitHub 反向代理）、`rsproxy.cn`（crates.io 镜像）
- 内网 License Server：`192.168.131.126:8089`（lmclient-rust-sdk git 依赖）

## 构建方式

### 方式一：本地 Bazel 构建

```bash
# 推荐：统一构建脚本（自动计算 fm.rNNN-HHHHHHHH 并注入 Bazel）
release/build-fm.sh --local

# 等价手动流程：
# 1. 启动 BCR registry proxy + 预下载依赖
# nohup python3 release/bazel-registry-proxy.py 8765 > /tmp/proxy.log 2>&1 &
# bash release/predownload-deps.sh
# 2. 计算后缀并构建（Bazel 沙箱没有 git 元数据，必须通过 action env 传入）
# suffix=fm.r37-456e4457
# bazel build --action_env=FM_BUILD_SUFFIX="$suffix" //codex-rs/cli:codex
# 3. 验证
# bazel-bin/codex-rs/cli/codex --version
# 输出: codex 0.146.0-fm.r37-456e4457
```

### 方式二：Docker 构建（推荐，用于分发）

```bash
release/build-fm.sh                        # 默认 Docker 构建，tag: codex:v<base>-fm.rNNN-HHHHHHHH-ubuntu-22.04
release/build-fm.sh --ubuntu-version 24.04
release/build-fm.sh --tag codex:custom
release/build-fm.sh --appimage            # 额外产出纯 CLI 单文件 AppImage（codex-<version>-x86_64.AppImage，无 desktop/icon；AppDir/打包全在容器 appimage stage 内完成）

# 提取二进制
id=$(docker create codex:ubuntu-22.04)
docker cp "$id:/usr/local/bin/codex" ./codex
docker rm "$id"
```

Docker 构建支持 `UBUNTU_VERSION` ARG 切换基础镜像版本（默认 22.04，可切 24.04）。
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

### lmclient 静态库链接

lmclient-rust-sdk 在源码树内打包了 `liblmclient.a`（CentOS 7 glibc 编译）。由于 rules_rust 的 sandbox 机制，build.rs 的 `cargo:rustc-link-search` 输出对 rustc 编译 action 不可见，因此用 `cc_import` + `cc_library` 替代：

- `third_party/lmclient/additive.BUILD.bazel` — 定义 cc_import 和系统库链接
- MODULE.bazel 的 `crate.annotation(gen_build_script = "off")` — 禁用 build.rs

### fmsh-ukey SDK 动态库链接

`fmsh-ukey-wrapper` 的 build.rs 会在上游仓库根目录找 `vendor/fmsh-ukey-sdk`，Bazel
沙箱里同样不可见。处理方式（与 lmclient 同思路）：

- `patches/fmsh_ukey_vendor_build.patch` — 给上游 git 仓库补
  `vendor/fmsh-ukey-sdk/linux/lib/BUILD.bazel`，把 SDK 目录标记为 Bazel 包；
- `third_party/fmsh-ukey/additive.BUILD.bazel` — 用 `cc_import` 链接
  `libfmsh_ukey_sdk.so`（必须用不带版本号的文件名，rules_rust 会按库名生成
  `-lfmsh_ukey_sdk`；若用 `libfmsh_ukey_sdk.so.0` 会在链接期找不到）；
- MODULE.bazel 的 `crate.annotation(crate = "fmsh-ukey-wrapper", gen_build_script = "off",
  patches = [...], deps = ["@crates//:fmsh_ukey_native"])` — 禁用 build.rs 并注入 native 依赖。

运行期说明：`libfmsh_ukey_sdk.so` 的 SONAME 是 `libfmsh_ukey_sdk.so.0`，其 NEEDED
`libcrypto.so.3` 由宿主 Ubuntu 22.04 的 libssl3 提供。Dockerfile 已把
`libfmsh_ukey_sdk.so.0` / GM3000 provider
`libgm3000.1.0.so` 打包到 `/usr/local/bin/lib`，且二进制 RUNPATH 包含
`$ORIGIN/lib`，镜像内外（二进制同级 `lib/` 目录）都能直接解析；GM3000
provider 是运行时 `dlopen` 加载，需通过 `FMSH_UKEY_PROVIDER` 指定其路径
（如 `/usr/local/bin/lib/libgm3000.1.0.so`）。开发环境按前文设置
`LD_LIBRARY_PATH=$PWD/vendor/fmsh-ukey-sdk/linux/lib` 即可。

## 版本号注入

版本号格式为 `0.146.0-fm.rNNN-HHHHHHHH`：

- `defs.bzl` 的 `WORKSPACE_VERSION`（=`0.146.0`）通过 `rust_library` / `rust_binary`
  的 `version` 属性注入 `CARGO_PKG_VERSION`，与 Cargo 构建保持一致；
- `codex-rs/cli/build.rs` 用 `git describe --tags --match 'rust-v[0-9]*'` 计算
  `fm.rNNN-HHHHHHHH`（NNN = 距最近 `rust-v*` tag 的 commit 数，HHHHHHHH = 短 hash）；
- Cargo 构建直接由 build.rs 计算；Bazel/Docker 沙箱内没有 git 元数据，由
  `release/build-fm.sh` 在构建前算好后通过 `FM_BUILD_SUFFIX` 环境变量 /
  `--build-arg` + `--action_env` 注入（build.rs 优先使用该覆盖值）。

## distdir 预缓存

`release/bazel-distdir/` 存放预下载的依赖归档。Bazel 的 `--distdir` 按 **sha256** 匹配文件（不看 URL），所以即使 MODULE.bazel 里的 URL 被改写过，distdir 里的原始归档也能命中。

`release/predownload-deps.sh` 负责下载那些 **BCR 模块内部** 引用的、无法通过 MODULE.bazel patch 修改的 github.com 归档（如 `bsdtar-prebuilt`、`bats-core`）。如果新版本出现新的未覆盖 URL，在此脚本中添加即可。

## Hermetic LLVM 工具链

Hermetic LLVM 指「自包含、可复现」的 LLVM 工具链：编译 C/C++ 时使用 Bazel 按固定版本
下载的 LLVM（Clang + lld + libc++/compiler-rt），不依赖构建机/宿主机上装了哪个版本的
gcc/clang，保证本机、CI、Docker 里构建行为完全一致。

仓库中的落地方式：

- `MODULE.bazel`：`bazel_dep(name = "llvm", version = "0.8.11")` +
  `register_toolchains("@llvm//toolchain:all")`，并用 `patches/llvm_*.patch`
  适配自定义 libc++ 与 Windows gnullvm/arm64 需求；
- `.bazelrc`：`BAZEL_DO_NOT_DETECT_CPP_TOOLCHAIN=1` /
  `BAZEL_NO_APPLE_CPP_TOOLCHAIN=1`，禁用宿主机 C/C++ 工具链探测；
- `BUILD.bazel`：目标平台标记为 glibc 2.28 兼容，产出 max GLIBC 2.28 的二进制；
  但 fmsh-ukey SDK 0.3.1 的共享库运行时需要 GLIBC_2.34，实际最低运行系统是
  Ubuntu 22.04（glibc 2.35）。

效果：

- V8、ICU、AWS-LC/OpenSSL 等 C/C++ 依赖统一由这套 clang/lld 编译，并静态链接进
  `codex` 二进制；
- 运行镜像不再需要构建机上的任何编译器，只需 glibc 和少数系统动态库
  （`libcurl4`、`libstdc++6`、`libzstd1`）以及 fmsh-ukey SDK；
- 同一份代码在不同环境构建结果一致，排查问题时不依赖「构建机装了哪个版本的工具链」。

## 产出规格

```
$ codex --version
codex 0.146.0-fm.r37-456e4457

$ file codex
ELF 64-bit LSB pie executable, x86-64, dynamically linked, interpreter /lib64/ld-linux-x86-64.so.2

$ readelf -V codex | grep -oP 'GLIBC_\K[0-9.]+' | sort -V | tail -1
2.28
```

- Cargo 发布构建（默认 `--sdk-link static`）：SDK + vendored libcrypto 静态
  链入，NEEDED 仅 `libcurl4`、`libstdc++6`（20.04+ 自带），glibc 上限 < 2.33
- `--sdk-link shared`（legacy）：动态链 SDK `.so`，max GLIBC 2.34（Ubuntu
  22.04+），依赖系统库 `libcurl4`、`libssl3`、`libstdc++6`、`libzstd1`
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
release/Dockerfile                          多阶段 Docker 构建（Ubuntu 22.04, BuildKit cache）
release/build-fm.sh                         统一构建脚本（计算版本后缀 + Docker/本地 Bazel）
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
