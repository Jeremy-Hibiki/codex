#!/usr/bin/env bash
# Pre-download archives that are fetched INSIDE BCR module extensions and
# therefore cannot be patched via MODULE.bazel overrides or ghfast URL
# rewrites. These are stored in release/bazel-distdir and served by Bazel's
# --distdir mechanism (matched by sha256, not URL).
#
# Known problematic modules (each has its own commit/version that may change):
#   - tar.bzl        → bsdtar-prebuilt
#   - bazel_lib      → bats-core
#   - rules_rs        → toml2json, bindgen, rules_rust archive
#
# If a new "github.com download failed" error appears in a clean Docker build,
# add the URL here.

set -euo pipefail

DISTDIR="${DISTDIR:-release/bazel-distdir}"
mkdir -p "$DISTDIR"
PROXY="https://ghfast.top/github.com"

download() {
    local url="$1"
    local filename
    filename="$(basename "$url")"
    local out="${DISTDIR}/${filename}"
    if [ -f "$out" ]; then
        return
    fi
    echo "  [downloading] $filename"
    curl -fsSL "${PROXY}/${url#https://github.com/}" -o "$out" || {
        echo "  [WARN] failed: $filename"
        rm -f "$out"
    }
}

# tar.bzl → bsdtar-prebuilt (used by llvm toolchain bootstrap)
download "https://github.com/hermeticbuild/bsdtar-prebuilt/releases/download/v3.8.1-3/tar_linux_amd64"

# bazel_lib → bats-core test framework
download "https://github.com/bats-core/bats-core/archive/v1.10.0.tar.gz"

echo "predownload-deps: done ($(ls "$DISTDIR" | wc -l) files in distdir)."
