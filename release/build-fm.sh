#!/usr/bin/env bash
#
# Build the FMSH (fm) Codex CLI with the `fm.rNNN-HHHHHHHH` version scheme.
#
# The suffix is derived from `git describe --tags --match 'rust-v[0-9]*'`
# (same logic as codex-rs/cli/build.rs), then handed to Bazel through the
# FM_BUILD_SUFFIX action env so the embedded `codex --version` matches the
# image tag even though the Bazel action sandbox has no git metadata.
#
# Usage:
#   release/build-fm.sh                          # docker build (default)
#   release/build-fm.sh --local                  # direct bazel build
#   release/build-fm.sh --appimage               # docker build + single-file AppImage
#   release/build-fm.sh --ubuntu-version 24.04   # base image override
#   release/build-fm.sh --suffix fm.r37-456e4457 # explicit suffix
#   release/build-fm.sh --tag codex:custom       # explicit image tag
#   release/build-fm.sh --base-version 0.146.0   # explicit base version
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

mode=docker
ubuntu_version=22.04
suffix_arg=""
tag_arg=""
base_version=""

usage() {
    sed -n '2,14p' "${BASH_SOURCE[0]}"
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --local)
            mode=local
            ;;
        --appimage)
            mode=appimage
            ;;
        --ubuntu-version)
            ubuntu_version="${2:?missing value for --ubuntu-version}"
            shift
            ;;
        --suffix)
            suffix_arg="${2:?missing value for --suffix}"
            shift
            ;;
        --tag)
            tag_arg="${2:?missing value for --tag}"
            shift
            ;;
        --base-version)
            base_version="${2:?missing value for --base-version}"
            shift
            ;;
        -h | --help)
            usage
            ;;
        *)
            echo "unknown argument: $1" >&2
            usage
            ;;
    esac
    shift
done

if [[ -z "$base_version" ]]; then
    base_version="$(awk '
        /^\[workspace\.package\]/ { in_section = 1 }
        in_section && /^version[[:space:]]*=/ {
            sub(/^version[[:space:]]*=[[:space:]]*"/, "")
            sub(/".*/, "")
            print
            exit
        }
    ' codex-rs/Cargo.toml)"
fi
[[ -n "$base_version" ]] || {
    echo "cannot determine base version from codex-rs/Cargo.toml" >&2
    exit 1
}

if [[ -n "$suffix_arg" ]]; then
    suffix="$suffix_arg"
else
    describe="$(git describe --tags --match 'rust-v[0-9]*' 2>/dev/null || true)"
    if [[ "$describe" == *-g* ]]; then
        hash="${describe##*-g}"
        hash="${hash:0:8}"
        rest="${describe%-g*}"
        rev="${rest##*-}"
        suffix="fm.r${rev}-${hash}"
    else
        rev=0
        hash="$(git rev-parse --short=8 HEAD 2>/dev/null || echo unknown)"
        suffix="fm.r${rev}-${hash}"
    fi
fi

version="${base_version}-${suffix}"
echo "base version: ${base_version}"
echo "build suffix: ${suffix}"
echo "codex version: ${version}"

FM_APPIMAGE_CONTAINER=""
cleanup_appimage() {
    if [[ -n "$FM_APPIMAGE_CONTAINER" ]]; then
        docker rm -f "$FM_APPIMAGE_CONTAINER" >/dev/null 2>&1 || true
    fi
}
trap cleanup_appimage EXIT

build_appimage() {
    local tag="$1"
    local version="$2"
    local out="$repo_root/codex-${version}-x86_64.AppImage"
    echo "== copying AppImage out of the appimage stage =="
    FM_APPIMAGE_CONTAINER="$(docker create "$tag")"
    docker cp "$FM_APPIMAGE_CONTAINER:/codex-${version}-x86_64.AppImage" "$out"

    echo "== verifying =="
    if "$out" --version 2>/dev/null; then
        :
    elif APPIMAGE_EXTRACT_AND_RUN=1 "$out" --version; then
        :
    else
        echo "AppImage verification failed" >&2
        return 1
    fi
    echo "AppImage: $out"
}

if [[ "$mode" == local ]]; then
    bazel build --action_env=FM_BUILD_SUFFIX="$suffix" //codex-rs/cli:codex
    bazel-bin/codex-rs/cli/codex --version
elif [[ "$mode" == docker || "$mode" == appimage ]]; then
    if [[ "$mode" == appimage ]]; then
        image_tag="codex-appimage:${version}"
        DOCKER_BUILDKIT=1 docker build \
            --build-arg HTTP_PROXY="${HTTP_PROXY:-}" \
            --build-arg HTTPS_PROXY="${HTTPS_PROXY:-}" \
            --build-arg NO_PROXY="${NO_PROXY:-}" \
            --build-arg UBUNTU_VERSION="$ubuntu_version" \
            --build-arg FM_BUILD_SUFFIX="$suffix" \
            --build-arg FM_BASE_VERSION="$base_version" \
            --target appimage \
            -t "$image_tag" \
            -f release/Dockerfile .
        build_appimage "$image_tag" "$version"
    else
        tag="${tag_arg:-codex:v${base_version}-${suffix}-ubuntu-${ubuntu_version}}"
        DOCKER_BUILDKIT=1 docker build \
            --build-arg HTTP_PROXY="${HTTP_PROXY:-}" \
            --build-arg HTTPS_PROXY="${HTTPS_PROXY:-}" \
            --build-arg NO_PROXY="${NO_PROXY:-}" \
            --build-arg UBUNTU_VERSION="$ubuntu_version" \
            --build-arg FM_BUILD_SUFFIX="$suffix" \
            -t "$tag" \
            -f release/Dockerfile .
        docker run --rm "$tag" --version
    fi
else
    echo "unknown mode: $mode" >&2
    exit 1
fi
