#!/usr/bin/env bash
# 内部全功能构建第一步:把 fmsh-ukey-* / lmclient-rust-sdk 的 workspace 依赖
# 从仓库内 placeholder 路径切回内部 GitLab git 依赖(需要能访问
# 192.168.131.126:8089)。之后用 --features ukey(fm-check / fm-test)构建。
#
# 恢复离线默认(placeholder):release/use-dummy-fm-deps.sh,
# 或 git checkout -- codex-rs/Cargo.toml codex-rs/Cargo.lock
#
# 注意:切换会改写 Cargo.lock(git 源条目),不要把切换后的 lock 提交进仓库。
set -eu
cd "$(dirname "$0")/../codex-rs"

python3 - <<'PYEOF'
import re

p = "Cargo.toml"
s = open(p).read()
real = {
    "lmclient-rust-sdk": 'lmclient-rust-sdk = { git = "http://192.168.131.126:8089/jiangzhengqi/lmclient-rust-sdk.git", tag = "v1.3.2" }',
    "fmsh-ukey-core": 'fmsh-ukey-core = { git = "http://192.168.131.126:8089/jiangzhengqi/fmsh-ukey-lib.git", tag = "0.5.1-sdk.0.3.2" }',
    "fmsh-ukey-sdk-wrapper": 'fmsh-ukey-sdk-wrapper = { git = "http://192.168.131.126:8089/jiangzhengqi/fmsh-ukey-lib.git", tag = "0.5.1-sdk.0.3.2" }',
    "fmsh-ukey-skill": 'fmsh-ukey-skill = { git = "http://192.168.131.126:8089/jiangzhengqi/fmsh-ukey-lib.git", tag = "0.5.1-sdk.0.3.2" }',
}
for name, line in real.items():
    s, n = re.subn(
        rf'^{name} = \{{ path = "vendor/dummy/{name}" \}}\n', line + "\n", s, flags=re.M
    )
    if n == 0 and f"{name} = {{ git" not in s:
        raise SystemExit(f"workspace dependency not found or unexpected form: {name}")
open(p, "w").write(s)
print("switched to real GitLab dependencies.")
print("build: --features ukey  (or cargo fm-check / cargo fm-test)")
print("remember: do not commit the rewritten Cargo.lock; restore with use-dummy-fm-deps.sh")
PYEOF
