#!/usr/bin/env bash
# 恢复离线默认:把 fmsh-ukey-* / lmclient-rust-sdk 的 workspace 依赖切回仓库内
# placeholder 路径(默认提交状态)。等同于 git checkout 还原清单与锁。
set -eu
cd "$(dirname "$0")/../codex-rs"
git checkout -- Cargo.toml Cargo.lock
echo "restored placeholder dependencies (offline default)."
