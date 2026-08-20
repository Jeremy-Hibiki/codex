# Agent Security 部署文档（Docker Compose + UKey）

> 面向“每用户一容器、非 root、无 SSH、仅 HTTP(S) API、持久化常驻”的部署模型。
> 本文档回答四个问题：① 示例 Docker Compose；② 需要挂载哪些路径；
> ③ 当前 bwrap 配置等价的命令行形式；④ docker compose 如何透传 USB 设备访问 UKey。

## 0. 部署模型回顾

- **执行容器**（本文档主体）：跑 `codex-app-server` + `codex-exec-server`，
  存放加密 skill 包（`.zip.enc`）与解密明文（`/dev/shm`），**不开 SSH**；
- **OpenChamber 容器**（用户 SSH + VSCode）：只经 HTTP(S) 与执行容器通信，
  **不得**挂载 skill 目录或 `/dev/shm`；
- 沙箱：`sandbox_mode = "workspace-write"`，Linux 下由 bwrap 实现，
  `network_access = false` 时 bwrap 追加 `--unshare-net`；
- 加密技能：`[encrypted_skills]`，`sdk = "ukey-two-phase"`（默认）时一次
  UKey 硬件调用解开 `key.enc`，AES 密钥仅驻内存；UKey 为 USB 硬件
  （GM3000 芯片），经 `FMSH_UKEY_PROVIDER` / `FMSH_UKEY_CONTAINER` 访问。

## 1. 示例 Docker Compose

镜像来自 `release/Dockerfile`（构建产物含 `codex`、`codex-app-server`、
`codex-exec-server`、`codex-linux-sandbox`、`codex-bwrap`。Cargo 发布构建
`release/build-fm-cargo.sh` 默认静态链入 fmsh-ukey SDK 与 libcrypto，
二进制 NEEDED 仅剩 `libstdc++.so.6` 与 glibc 基础库（lmclient v1.3.0 起 curl 亦为 vendored 静态；目标平台 Ubuntu 22.04+），
`$ORIGIN/lib` 里只需放 dlopen 的 GM3000 provider，见 `release/BUILD.md`）。
以下为单用户执行容器示例：

```yaml
# docker-compose.yml（每用户一份；<user> 替换为实际用户名）
services:
  codex-agent:
    image: codex:ubuntu-20.04
    container_name: codex-agent-<user>
    restart: unless-stopped
    environment:
      HOME: /home/codex
      CODEX_HOME: /home/codex/.codex
      # FMSH License（进程启动校验；缺了 app-server 直接退出，见第 5 节）
      FMSH_LIC_SERVER: "27000@10.0.0.10"
      FMSH_CODEX_LIC_FEATURE: "Codex"
      FMSH_CODEX_LIC_VERSION: "1.0"
      # FMSH_CODEX_LIC_DISPLAY_NAME: "Codex"   # 可选，默认 "Codex"
      # FMSH_CODEX_LIC_HOSTNAME: "agent-01"   # 可选，上报给 License 服务端的客户端主机名，默认走 SDK 内部值
      # UKey 运行时（不落配置，只走环境变量）
      FMSH_UKEY_PROVIDER: /opt/fmsh-ukey/lib/libgm3000.1.0.so
      FMSH_UKEY_CONTAINER: ukey0
      # FMSH_UKEY_SDK_DIR 是构建期变量，运行期不需要
    volumes:
      # 用户工作区（bwrap 的 writable root）
      - ./workspaces/<user>:/workspace
      # 用户配置（只读）
      - ./config/<user>/config.toml:/home/codex/.codex/config.toml:ro
      # 受管策略（只读，覆盖用户配置）
      - ./config/requirements.toml:/etc/codex/requirements.toml:ro
      # 审计日志持久卷（非 root 可写，容器重启不丢）
      - ./data/<user>/audit:/data/codex/audit
      # 加密 skill 包（仅执行容器私有；不得暴露给 OpenChamber 容器）
      - ./data/<user>/skills:/home/codex/.codex/skills
      # UKey provider / SDK 动态库（只读）
      - ./ukey/lib:/opt/fmsh-ukey/lib:ro
    devices:
      # USB 透传访问 UKey（见第 4 节）
      - /dev/bus/usb:/dev/bus/usb
    # /dev/shm 容量：按技能包估算，建议 >= 256MB（容量门控预留 >= 4MiB）
    shm_size: 512m
    # 可选：有 root 时允许 mlock 常驻（无 root 部署可省略）
    ulimits:
      memlock:
        soft: -1
        hard: -1
    user: "1000:1000"
    # 可选：宿主 plugdev/dialout 组，让非 root 用户访问 USB 设备节点
    group_add:
      - "44"
    ports:
      - "127.0.0.1:18080:8080"   # app-server websocket API（仅本机可达）
    # 入口：启动 app-server（--listen ws://0.0.0.0:8080）并拉起配套 exec-server
    # （exec-server 监听端口与启动方式随版本演进，按实际部署入口脚本配置）
    command: ["/usr/local/bin/codex-app-server", "--listen", "ws://0.0.0.0:8080"]
```

要点：

- `restart: unless-stopped`：常驻服务；容器销毁重建后 `/dev/shm` 是全新
  tmpfs，解密明文自动消失（安全属性，不要给它挂持久卷）；
- 网络策略：容器 `--network none` 或出口白名单；模型 API 由宿主侧进程发起，
  不经 bwrap；局域网推理服务走白名单；
- bwrap 依赖 user namespace：宿主需允许非特权 user namespace
  （`kernel.unprivileged_userns_clone=1`，AppArmor 不拦截）；
- 双容器拓扑：OpenChamber 容器是独立的第二个 service，只暴露 HTTP 端口，
  不挂载 `/home/codex/.codex/skills` 与 `/dev/shm`；
- 端口只绑 `127.0.0.1`，如需对 OpenChamber 容器开放，改走内网 overlay 网络
  并配置认证。

### bwrap 需要哪些能力？其实不需要 CAP

bwrap 的设计就是**不需要 `CAP_SYS_ADMIN`**（也不依赖容器特权）：它先进入新的
user namespace（`--unshare-user`），在**新 userns 内部**获得完整能力后再创建
mount/PID/net 命名空间——父命名空间里不需要任何能力。Codex 显式传
`--unshare-user`，因为 bwrap 的自动开启逻辑在调用者是 uid 0 时会被跳过
（`linux-sandbox/src/bwrap.rs` 注释原文），所以容器以 root 跑也一样不依赖
宿主 CAP。

真正的前提是“**能创建非特权 user namespace**”，三处都要满足：

| 检查项         | 要求                                        | 说明                                                                                |
|----------------|---------------------------------------------|-------------------------------------------------------------------------------------|
| 宿主内核       | `kernel.unprivileged_userns_clone = 1`      | Debian/Ubuntu 默认 1；0 则任何容器都建不了 userns                                   |
| 宿主 LSM       | `apparmor_restrict_unprivileged_userns = 0` | Ubuntu 23.10+ 默认 1，会拦非特权 userns，需关掉或给容器配 profile                   |
| Docker seccomp | 放行 `unshare()` / `mount()`                | 默认 profile 一般放行；若被拦，`--security-opt seccomp=unconfined` 或自定义 profile |

实测（本容器：非 root、`CapEff=0`、无 seccomp）：

```bash
unshare --user --map-root-user --mount true        # OK：无需任何能力
bwrap --ro-bind / / --dev /dev \
      --unshare-user --unshare-pid --unshare-net \
      --proc /proc /bin/true                        # FAIL: Can't mount proc ... EPERM
bwrap --ro-bind / / --dev /dev \
      --unshare-user --unshare-pid --unshare-net \
      /bin/true                                     # OK：去掉 --proc 后全可用
```

即：**受限容器里 `--proc /proc` 可能被内核拒绝，但其余命名空间照常工作**——
这正是 helper `--no-proc` 开关的用途（产品默认仍挂 `/proc`）。另外实测沙箱内
进程 `CapEff = 0`，能力在进入沙箱时被清空。

如果宿主连 userns 都不给（企业安全基线常见），兜底顺序：

1. 容器加 `--security-opt seccomp=unconfined` + 宿主开 userns；
2. 接受 guard-only（bwrap 不可用告警）并收紧容器级网络策略；
3. `privileged: true` 是最省事的方案，但安全面最大，只在内部可信部署用。

### mlock 需要什么？daemon 设置 ulimits 即可，不需要特权

mlock 与 CAP 无关，只受 `RLIMIT_MEMLOCK` 约束：

- 默认容器 `RLIMIT_MEMLOCK` 硬限制 8 MiB（实测 `ulimit -l` = 8192 KB）；单技能包明文上限 16 MiB，**无 root 且不提限制时无法全量 pin**；
- 非 root 进程**不能自己提升硬限制**（实测 `ulimit -l unlimited` → `Operation not permitted`）；
- 解法就是 compose 的 `ulimits: memlock: {soft: -1, hard: -1}`：由 Docker daemon（root）在**容器创建时**设置 rlimit，容器内非 root 进程无需任何能力即可看到 unlimited，之后 `mlock()` 无需 `CAP_IPC_LOCK`（限制内锁定即可）。

实测（当前限制下）：`mlock` 4 MiB 成功；12 MiB 超限返回 `ENOMEM`。

设置 `ulimits` 后这两条都应成功。`--privileged` 对提升 memlock **不是必需

## 2. 需要挂载哪些路径

| 路径           | 方式                | 容器内位置                     | 说明                                                                                         |
|----------------|---------------------|--------------------------------|----------------------------------------------------------------------------------------------|
| 用户工作区     | bind 卷             | `/workspace`                   | 项目根，bwrap `--bind` 可写根                                                                |
| 用户配置       | 只读 bind           | `~/.codex/config.toml`         | 用户级配置                                                                                   |
| 受管策略       | 只读 bind           | `/etc/codex/requirements.toml` | `allowed_sandbox_modes`、`[encrypted_skills]` 等受管项                                       |
| 审计日志       | 持久卷              | `/data/codex/audit`            | 非 root 可写；10MB 轮转；重启保留                                                            |
| skill 包目录   | 持久卷（私有）      | `~/.codex/skills`              | `<name>.zip.enc` 密文；**不得**暴露给 OpenChamber 容器                                       |
| UKey provider  | 只读 bind           | `/opt/fmsh-ukey/lib`           | GM3000 provider（`libgm3000.1.0.so`）；SDK 本体已静态链入二进制，无需挂 SDK/libcrypto 动态库 |
| USB 设备       | devices 透传        | `/dev/bus/usb`                 | UKey 硬件访问（第 4 节）                                                                     |
| `/dev/shm`     | tmpfs（`shm_size`） | `/dev/shm`                     | 解密明文；**不挂持久卷**，容器重启自动清空                                                   |
| 镜像自带动态库 | 镜像内              | `$ORIGIN/lib`                  | codex 二进制 rpath 指向，勿覆盖                                                              |

**明确不要挂载**：

- `/dev/shm` 不要做成持久卷（明文残留依赖“重启即清空”的安全属性）；
- `~/.codex/skills` 与 `/dev/shm` 不要挂进 OpenChamber 容器（用户有 SSH，双挂载等于把加密包和明文交给用户）；
- skill 包目录权限 0700、容器内用户私有。

## 3. 当前 bwrap 配置等价的命令行形式

部署推荐配置（`config.toml`）：

```toml
sandbox_mode = "workspace-write"

[sandbox_workspace_write]
writable_roots = ["./", "/dev/shm/fm-agent-security"]
network_access = false
```

对应 Codex 构造出的 bwrap 参数（由 `linux-sandbox/src/bwrap.rs`, `create_bwrap_flags` + `create_filesystem_args` 生成，挂载顺序即下方顺序）：

```bash
bwrap \
  --new-session --die-with-parent \
  --ro-bind / / \                       # workspace-write：整盘只读基线
  --dev /dev \                          # 最小设备树：null/zero/random/urandom/tty
  --unshare-user --unshare-pid \
  --unshare-net \                       # network_access=false
  --proc /proc \                        # 受限容器被拒时 helper 可用 --no-proc
  --chdir /workspace/proj \             # 沙箱内逻辑 cwd
  --bind /workspace/proj /workspace/proj \           # writable root：项目根
  --bind /tmp /tmp \                                  # 默认 tmp 可写
  --bind /dev/shm/fm-agent-security /dev/shm/fm-agent-security \
  --ro-bind /workspace/proj/.git /workspace/proj/.git \     # 保护元数据重遮罩
  --ro-bind /workspace/proj/.agents /workspace/proj/.agents \
  --ro-bind /workspace/proj/.codex /workspace/proj/.codex \
  --ro-bind /dev/shm/codex-skills/<hash> /workspace/proj/.codex/skills/<name> \
  -- bash -lc "<用户命令>"
```

实际运行走 `codex-linux-sandbox` helper（等价形式，`readonly_binds` 以
`--ro-bind SOURCE TARGET` 成对传入）：

```bash
codex-linux-sandbox \
  --sandbox-policy-cwd /workspace/proj \
  --command-cwd /workspace/proj \
  --permission-profile '<权限档案 JSON>' \
  --ro-bind /dev/shm/codex-skills/<hash> /workspace/proj/.codex/skills/<name> \
  -- bash -lc "<用户命令>"
```

`--permission-profile` 传的是序列化后的 `PermissionProfile` JSON
（`linux_run_main.rs` 用 `serde_json::from_str` 解析）。

配置到参数的映射：

| 配置项                                            | bwrap 参数                                                                  | 说明                            |
|---------------------------------------------------|-----------------------------------------------------------------------------|---------------------------------|
| `sandbox_mode = "workspace-write"`                | `--ro-bind / /` + `--dev /dev`                                              | 整盘只读基线 + 最小设备树       |
| 其他 restricted（无整盘读）                       | `--tmpfs /` + `--dev /dev` + 逐个 `--ro-bind` 可读根                        | 从空文件系统起步                |
| `writable_roots = ["./"]`                         | `--bind <workspace> <workspace>`                                            | 项目根可写                      |
| `writable_roots = ["/dev/shm/fm-agent-security"]` | `--bind /dev/shm/fm-agent-security ...`                                     | 解密脚本可执行区                |
| `network_access = false`                          | `--unshare-net`                                                             | 沙箱内断网                      |
| 默认强制                                          | `--new-session --die-with-parent --unshare-user --unshare-pid --proc /proc` | 用户/PID 命名空间、全新 `/proc` |
| 保护元数据 `.git/.agents/.codex`                  | `--ro-bind <子路径> <子路径>`                                               | 在可写根上重遮罩                |
| 解密技能 readonly_binds                           | 最后 `--dir <target>`（缺失时）+ `--ro-bind <src> <target>`                 | 覆盖可写根；source 缺失则跳过   |

行为要点：

- 挂载顺序：base mounts → writable roots → read-only subpaths / unreadable
  masks → readonly_binds（最后应用，所以技能只读视图“赢”过可写根）；
- `--dev /dev` 使沙箱内 `/dev/shm` 是**私有空 tmpfs**，宿主标记不可见；
- full-disk-write + 全网络 + 无 unreadable globs 时**跳过 bwrap**（本部署
  的 workspace-write + `network_access=false` 不会触发跳过）；
- `--proc /proc` 在某些受限容器内被拒，此时 helper 传 `--no-proc`，但产品
  路径默认仍挂载；
- 沙箱内看不到 USB 设备（`--dev /dev` 只有最小节点）——UKey 解密发生在
  codex 宿主进程（容器主进程），不在沙箱命令内，见第 4 节。

## 4. docker compose 挂载 USB 设备访问 UKey

### 访问模型

UKey 是 USB 硬件（GM3000 芯片），SDK 经 `FMSH_UKEY_PROVIDER`（provider 动态库，
如 `libgm3000.1.0.so`）与 `FMSH_UKEY_CONTAINER`（UKey 容器名）访问。解密发生在
**codex 宿主进程**（skill 加载时的一次/两阶段调用），不发生在 bwrap 沙箱内；
因此 USB 设备只需透传给容器主进程，沙箱命令（`--dev /dev` 最小设备树）不需要、
也看不到 UKey。

### compose 写法

```yaml
services:
  codex-agent:
    devices:
      - /dev/bus/usb:/dev/bus/usb
```

等价于 `docker run --device /dev/bus/usb:/dev/bus/usb`。现代 Docker 支持目录
透传（对目录下的字符设备生成 cgroup 规则）。若你的 Docker 版本不支持目录
`--device`，改用具体设备节点（见下）。

### 具体设备节点（旧 Docker / 精确控制）

```bash
# 在宿主机上找 UKey
lsusb
# 假设 Bus 001 / Device 009
docker run --device /dev/bus/usb/001/009:/dev/bus/usb/001/009 ...
```

compose 写法：

```yaml
    devices:
      - /dev/bus/usb/001/009:/dev/bus/usb/001/009
```

### 热插拔与宽松方案

- 目录透传后，热插拔的新设备节点仍在 cgroup 允许范围内，通常无需重启；
- 个别宿主机（udev 权限收紧、多 UKey）可直接绑定整个 `/dev` 或用
  `privileged: true`（最宽松，安全面最大，不推荐）；
- 若 UKey 暴露 HID 接口（`/dev/hidraw*`），按需追加绑定：

```yaml
    devices:
      - /dev/bus/usb:/dev/bus/usb
      - /dev/hidraw0:/dev/hidraw0   # 按宿主机实际枚举
```

### 容器内非 root 用户的设备权限

`/dev/bus/usb/*/*` 在多数发行版是 `root:root 0666`（usbfs），可直接访问；
若为 `0664 root:plugdev` 之类，则：

- compose 加 `group_add` 对应宿主 GID（如 `44` = plugdev、`20` = dialout），
  或
- 宿主机加 udev 规则把 UKey 设备节点 chmod/chown 放开。

### 验证

```bash
docker exec codex-agent-<user> lsusb          # 应能看到 UKey
docker exec codex-agent-<user> ls -l /dev/bus/usb/*/*
# 日志里不应出现 SdkUnavailable / HardwareKeyRequired
docker logs codex-agent-<user> | rg -i "ukey|sdk|encrypted"
```

## 5. License 相关环境变量

`fm-license` crate（`codex-rs/fm/license/`）在**进程启动时**向 FMSH
LicenseService checkout 一次，持有到进程结束；校验失败进程直接退出，运行时
心跳耗尽则拒绝“发起新工作”的请求（JSON-RPC `-32002`）但不杀进程。所有能启动
agent 的入口（`codex` / `codex exec` / `codex app-server` / `codex mcp-server`
/ `codex-exec` 等）都会校验；`codex cloud`、`exec-server` 不占用 license。

### 启动必需（缺失即失败）

| 变量                          | 必填 | 格式/默认       | 说明                                                                    |
|-------------------------------|------|-----------------|-------------------------------------------------------------------------|
| `FMSH_LIC_SERVER`             | 是   | `<port>@<host>` | LicenseService 地址；LMCLIENT SDK 内部读取（Codex 代码不直接读）        |
| `FMSH_CODEX_LIC_FEATURE`      | 是   | 特性名          | 缺失时 `resolve_config` 直接报错                                        |
| `FMSH_CODEX_LIC_VERSION`      | 是   | 特性版本        | 缺失时直接报错                                                          |
| `FMSH_CODEX_LIC_DISPLAY_NAME` | 否   | 默认 `"Codex"`  | 显示名                                                                  |
| `FMSH_CODEX_LIC_HOSTNAME`     | 否   | SDK 默认        | 客户端主机名（`LM_INIT_STRUCT.hostName`），License 服务端侧的客户端标识 |

### 测试 / 运维开关

| 变量                             | 取值                          | 说明                                                                           |
|----------------------------------|-------------------------------|--------------------------------------------------------------------------------|
| `FMSH_CODEX_LIC_TEST_BYPASS`     | `1`/`true`/`TRUE`/`yes`/`YES` | 跳过校验；**release 构建同样生效**（产品级开关），CI / 无 LicenseServer 环境用 |
| `FMSH_CODEX_LIC_TEST_FORCE_LOST` | 同上，须与 BYPASS 同置        | 启动即置 Lost 状态，用于测请求门禁                                             |
| `CODEX_THREAD_ID`                | Codex 注入                    | 嵌套会话标记：Codex 会话内再起 Codex 时跳过 checkout，不占第二个 seat          |

### 相邻但非 License（同部署面）

| 变量                                       | 时机       | 说明                                                                                                         |
|--------------------------------------------|------------|--------------------------------------------------------------------------------------------------------------|
| `FMSH_UKEY_SDK_LINK`                       | **构建期** | `static`（发布默认）：SDK 归档 + vendored libcrypto 静态链入；`shared`：动态链 SDK `.so`（需 Ubuntu 22.04+） |
| `FMSH_UKEY_LIBSTDCPP`                      | **构建期** | static 模式下 `shared`（发布默认）：libstdc++ 动态链，避免与 V8 内嵌 libc++abi 的 `__cxa_*` 冲突             |
| `FMSH_UKEY_STATIC_DEDUP_AGAINST`           | **构建期** | static 模式下指向 `liblmclient.a`，自动去重 lmclient 与 SDK 共享的工具层符号（构建脚本自动设置）             |
| `FMSH_UKEY_PROVIDER`                       | 运行期     | UKey provider 动态库路径（如 `libgm3000.1.0.so`）                                                            |
| `FMSH_UKEY_CONTAINER`                      | 运行期     | UKey 容器名                                                                                                  |
| `FMSH_CODEX_AGENT_SECURITY_SANDBOX_BYPASS` | debug 构建 | 沙箱 debug 后门；release 构建恒忽略                                                                          |

部署注意：每容器一个 Codex 进程 = 一个 license seat；SIGINT/SIGTERM 会先
`check_in` 归还（`lmCheckIn`+`lmExit`）。变量缺失导致的启动失败会表现为
容器反复重启，日志里有 `license checkout failed`。

## 6. AppArmor / seccomp / ulimits：各管各的，互不替代

三层机制作用域完全不同，**配了其中任何一个都不能替代另外两个**：

| 机制             | 作用对象                                        | 谁设置                                 | 管什么                      |
|------------------|-------------------------------------------------|----------------------------------------|-----------------------------|
| AppArmor profile | 宿主原生进程，或容器（`docker-default`/自定义） | 宿主 LSM + Docker `security_opt`       | 文件/网络/mount 等 LSM 策略 |
| seccomp          | 容器内全部系统调用                              | Docker `security_opt`                  | 系统调用放行/拦截           |
| RLIMIT_MEMLOCK   | 容器内进程的 `mlock()` 上限                     | Docker daemon（`ulimits`，容器创建时） | 可锁内存字节数              |

### bwrap-userns-restrict 与 apparmor=unconfined：冗余

- `bwrap-userns-restrict` 是 Ubuntu 24.04+（`apparmor_restrict_unprivileged_userns=1`
  开启时）宿主侧的 profile，给宿主**原生**运行的 `/usr/bin/bwrap` 放行非特权 user
  namespace；其内容几乎是"全放行"，唯一目的就是让 bwrap 在受限制 userns 的宿主上能跑。
- compose 设了 `apparmor=unconfined` 后，容器内 bwrap 不受宿主 AppArmor 策略管辖，
  **该 profile 对容器内 bwrap 不生效**。它的价值只在宿主原生跑 bwrap 的场景
  （例如装 SDK 时在容器外执行 bwrap），与容器内部署无关。
- 若希望容器内也受 AppArmor 约束，应使用 Docker 侧 profile（默认 `docker-default`
  或自定义 profile 经 `--security-opt apparmor=<profile>` 指定），而不是宿主 profile。

### seccomp=unconfined：不建议全关

`seccomp=unconfined` 能排除 docker-default 对 `unshare/mount` 之类调用的限制，但代价是
容器内 app-server 进程失去默认系统调用保护——对 agent-security 场景是安全减分。另外
**实测**（本容器 Seccomp=0 即已 unconfined）：`bwrap --proc /proc` 仍 EPERM，说明
`--proc` 被拒不一定来自 seccomp，**unconfine 不保证修复**。建议：

1. 优先保留默认 seccomp；`--proc` 失败走 helper `--no-proc`（产品已有开关）；
2. 确实需要挂载 proc 时，用自定义最小 seccomp profile 放行所需调用，而不是全 unconfine；
3. 仅内部可信部署、且确认 unconfine 是唯一解时才用。

### ulimits（memlock）：与 AppArmor/seccomp 正交，按需仍要配

`RLIMIT_MEMLOCK` 由 Docker daemon 在容器创建时设置，AppArmor/seccomp 完全不涉及；
非 root 进程无法自行提升硬限制（实测 `ulimit -l unlimited` → EPERM）。因此：

- **要 mlock 驻留解密明文**（单包明文上限 16MiB > 默认 8MiB）：**仍需要**
  `ulimits: memlock: {soft: -1, hard: -1}`，与是否 unconfine AppArmor/seccomp 无关；
- 接受明文可能被换出到 swap（威胁模型外）：可省略，此时 mlock 只锁住部分页面。

一句话结论：**AppArmor profile、seccomp、ulimits 三者用途互不重叠，该配 memlock 的
仍要配。**

## 7. 验证清单（部署后）

1. bwrap 可用：容器内 `bwrap --ro-bind / / --dev /dev --unshare-user
   --unshare-pid --unshare-net /bin/true` 成功（若仅 `--proc /proc` 失败属
   受限容器常见现象，可接受 guard-only + 网络收紧，或配 `--no-proc`；不要为
   此单独开 `seccomp=unconfined`，实测 unconfine 不保证修复）；
2. 宿主确认 `sysctl kernel.unprivileged_userns_clone=1` 且
   `kernel.apparmor_restrict_unprivileged_userns=0`；
3. mlock：`ulimit -l` 应为 `unlimited`（compose 已设 `ulimits.memlock`），
   否则按上文无法 pin 全部明文；
4. `/dev/shm` 容量 `df -h /dev/shm` >= 256MB 且非 root 可写；
5. 审计日志写入 `/data/codex/audit`，重启后保留；
6. skill 包只在执行容器私有路径，OpenChamber 容器无该挂载；
7. `lsusb` 可见 UKey，`FMSH_UKEY_PROVIDER` 路径存在且可读；
8. license 变量齐备：`FMSH_LIC_SERVER`/`FMSH_CODEX_LIC_FEATURE`/
   `FMSH_CODEX_LIC_VERSION` 已设置，日志无 `license checkout failed`；
9. 网络：容器无外网，推理服务经白名单可达。

## 参考

- bwrap 参数构造：`codex-rs/linux-sandbox/src/bwrap.rs`
  （`create_bwrap_flags` / `create_filesystem_args`）
- helper CLI：`codex-rs/linux-sandbox/src/linux_run_main.rs`（`LandlockCommand`）
- readonly_binds 判定：`codex-rs/fm/encrypted-skills/src/sandbox_policy.rs`
- License 实现与入口校验：`codex-rs/fm/license/README.md`、
  `codex-rs/fm/license/src/license.rs`
- UKey SDK 接入分析：`fm-docs/FMSH_UKEY_INTEGRATION_ANALYSIS.md`
- 部署配置示例与验证清单：`fm-docs/FM_AGENT_SECURITY_IMPLEMENTATION_REPORT_V2.md` 第 16 节
