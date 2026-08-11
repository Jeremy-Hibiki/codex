#!/usr/bin/env bash
#
# Build the FMSH (fm) Codex CLI using Cargo + Docker.
#
# This is the Cargo counterpart to release/build-fm.sh (which uses Bazel).
# It mirrors the same CLI interface (--local / --docker / --appimage /
# --suffix / --tag / --ubuntu-version / --base-version) so it can be used
# as a drop-in replacement.
#
#   release/build-fm-cargo.sh                       # docker build (default)
#   release/build-fm-cargo.sh --local               # direct cargo build (release)
#   release/build-fm-cargo.sh --local --debug       # debug build (no strip, with symbols)
#   release/build-fm-cargo.sh --appimage            # docker + single-file AppImage
#   release/build-fm-cargo.sh --ubuntu-version 22.04
#   release/build-fm-cargo.sh --suffix fm.r37-456e4457
#   release/build-fm-cargo.sh --tag codex:custom
#   release/build-fm-cargo.sh --base-version 0.146.0
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

mode=docker
ubuntu_version=20.04
suffix_arg=""
tag_arg=""
profile=release

usage() {
 sed -n '2,18p' "${BASH_SOURCE[0]}"
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
 --debug)
  profile=debug
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

# ── Resolve version ────────────────────────────────────────────────────────

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
 describe="$(git describe --tags --match 'rust-v[0.9]*' 2>/dev/null || true)"
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
echo "profile: $profile"
# ── Locate the fmsh-ukey SDK lib directory (for --local) ───────────────────
# The SDK is vendored inside the fmsh-ukey-lib git checkout. Cargo resolves
# this automatically; we only need the path for the local-build .so bundling.

find_sdk_lib_dir() {
 local sdk_dir
 # From cargo checkout
 sdk_dir="$(find ~/.cargo/git/checkouts/fmsh-ukey-lib-* \
  -path '*/vendor/fmsh-ukey-sdk/linux/lib' 2>/dev/null | head -1)"
 if [[ -z "$sdk_dir" ]]; then
  # From FMSH_UKEY_SDK_DIR env or a sibling repo
  if [[ -n "${FMSH_UKEY_SDK_DIR:-}" ]] && [[ -d "$FMSH_UKEY_SDK_DIR/linux/lib" ]]; then
   sdk_dir="$FMSH_UKEY_SDK_DIR/linux/lib"
  fi
 fi
 echo "$sdk_dir"
}

# ── Local build ────────────────────────────────────────────────────────────

build_local() {
 local codex_src="$repo_root/codex-rs"
 local out_dir="$repo_root/dist"
 local bin="$out_dir/codex"

 local cargo_profile_flag="--release"
 local target_subdir="release"
 if [[ "$profile" == "debug" ]]; then
  cargo_profile_flag=""
  target_subdir="debug"
 fi
 echo "== cargo build $cargo_profile_flag =="
 (
  cd "$codex_src"
  FM_BUILD_SUFFIX="$suffix" cargo build $cargo_profile_flag -p codex-cli --timings
 )

 mkdir -p "$out_dir/lib"

 cp "$codex_src/target/$target_subdir/codex" "$bin"
 if [[ "$profile" == "release" ]]; then
  echo "== stripping binary =="
  strip --strip-debug --strip-unneeded "$bin"
 else
  echo "== debug build: keeping symbols =="
 fi

 echo "== bundling fmsh-ukey SDK libs =="
 local sdk_lib_dir
 sdk_lib_dir="$(find_sdk_lib_dir)"
 if [[ -n "$sdk_lib_dir" ]]; then
  cp -fL "$sdk_lib_dir"/libfmsh_ukey_sdk.so.0 "$out_dir/lib/" 2>/dev/null || true
  cp -fL "$sdk_lib_dir"/libcrypto.so.1.1 "$out_dir/lib/" 2>/dev/null || true
  cp -fL "$sdk_lib_dir"/libgm3000.1.0.so "$out_dir/lib/" 2>/dev/null || true
  echo "  SDK libs: $sdk_lib_dir → $out_dir/lib/"
 else
  echo "  WARNING: fmsh-ukey SDK lib dir not found — .so files not bundled" >&2
  echo "  Set FMSH_UKEY_SDK_DIR or build via Docker (default mode)." >&2
 fi

 echo "== verifying =="
 # License gate requires env vars; just check --version with them stubbed.
 FMSH_CODEX_LIC_FEATURE=x FMSH_CODEX_LIC_VERSION=x \
  LD_LIBRARY_PATH="$out_dir/lib" \
  "$bin" --version 2>/dev/null || true

 echo ""
 echo "Build complete:"
 echo "  binary: $bin"
 echo "  libs:   $out_dir/lib/"
 echo "  version: $(FMSH_CODEX_LIC_FEATURE=x FMSH_CODEX_LIC_VERSION=x LD_LIBRARY_PATH=\"$out_dir/lib\" \"$bin\" --version 2>&1 || echo '(license gate active)')"
}

# ── Docker build ───────────────────────────────────────────────────────────

build_docker() {
 local tag="${tag_arg:-codex:${version}-ubuntu-${ubuntu_version}}"

 echo "== docker build =="
 DOCKER_BUILDKIT=1 docker build \
  --build-arg HTTP_PROXY="${HTTP_PROXY:-}" \
  --build-arg HTTPS_PROXY="${HTTPS_PROXY:-}" \
  --build-arg NO_PROXY="${NO_PROXY:-}" \
  --build-arg UBUNTU_VERSION="$ubuntu_version" \
  --build-arg FM_BUILD_SUFFIX="$suffix" \
  --build-arg CARGO_PROFILE="${profile:-release}" \
  -f release/Dockerfile.cargo \
  "$repo_root"

 echo "== verifying =="
 docker run --rm \
  -e FMSH_CODEX_LIC_FEATURE=x \
  -e FMSH_CODEX_LIC_VERSION=x \
  "$tag" --version

 echo ""
 echo "Image: $tag"
 echo ""
 echo "Extract binary:"
 echo "  id=\$(docker create $tag)"
 echo "  docker cp \"\$id:/usr/local/bin/codex\" ./codex"
 echo "  docker cp \"\$id:/usr/local/bin/lib\" ./lib"
 echo "  docker rm \"\$id\""
}

# ── AppImage build ─────────────────────────────────────────────────────────

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

 echo "== docker build (appimage stage) =="
 DOCKER_BUILDKIT=1 docker build \
  --build-arg HTTP_PROXY="${HTTP_PROXY:-}" \
  --build-arg HTTPS_PROXY="${HTTPS_PROXY:-}" \
  --build-arg NO_PROXY="${NO_PROXY:-}" \
  --build-arg UBUNTU_VERSION="$ubuntu_version" \
  --build-arg FM_BUILD_SUFFIX="$suffix" \
  --build-arg FM_BASE_VERSION="$base_version" \
  -t codex-appimage-tmp \
  -f release/Dockerfile.cargo \
  "$repo_root"

 # Build the AppImage from the Docker image's binary + libs.
 echo "== assembling AppImage =="
 FM_APPIMAGE_CONTAINER="$(docker create codex-appimage-tmp)"
 local stage="$repo_root/dist/appimage-stage"
 mkdir -p "$stage/app/usr/bin" "$stage/app/usr/lib"

 docker cp "$FM_APPIMAGE_CONTAINER:/usr/local/bin/codex" "$stage/app/usr/bin/codex"
 docker cp "$FM_APPIMAGE_CONTAINER:/usr/local/bin/lib/." "$stage/app/usr/lib/"

 # Collect closure libs (skip glibc) for a portable AppImage.
 docker --log-level=none run --rm --entrypoint bash codex-appimage-tmp \
  'ldd /usr/local/bin/codex \
         | awk -F"=> " "/=> \// {print \$2}" \
         | awk "{print \$1}" | sort -u \
         | while read -r lib; do
             case "\$lib" in
                 */ld-linux*|*/libc.so.6|*/libm.so.6|*/libpthread.so.0 \
                 |*/libdl.so.2|*/librt.so.1|*/libutil.so.1|*/libresolv.so.2 \
                 |*/libfmsh_ukey_sdk.so*|*/libcrypto.so.1.1) continue ;;
             esac
             cp -L "\$lib" "'"$stage"'/app/usr/lib/" 2>/dev/null || true
           done' || true

 # AppRun launcher
 cat >"$stage/app/AppRun" <<'RUNEOF'
#!/bin/sh
SELF="$(readlink -f "$0")"
APPDIR="${SELF%/*}"
export LD_LIBRARY_PATH="$APPDIR/usr/lib:$APPDIR/usr/bin/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$APPDIR/usr/bin/codex" "$@"
RUNEOF
 chmod +x "$stage/app/AppRun" "$stage/app/usr/bin/codex"

 # Download AppImage tooling through proxy if needed.
 local gh_proxy="${ghfast_top_proxy:-https://ghfast.top/github.com}"
 echo "== packing AppImage =="
 (
  cd "$stage"
  curl -fsSL "$gh_proxy/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage" \
   -o appimagetool
  chmod +x appimagetool
  curl -fsSL "$gh_proxy/AppImage/type2-runtime/releases/download/continuous/runtime-x86_64" \
   -o runtime

  APPIMAGE_EXTRACT_AND_RUN=1 ./appimagetool --appimage-extract >/dev/null 2>&1
  ./squashfs-root/usr/bin/mksquashfs app "$out" \
   -comp zstd -b 131072 -noappend >/dev/null 2>&1
  cat runtime >>"$out"
 )

 chmod +x "$out"
 rm -rf "$stage"

 echo "== verifying =="
 "$out" --version 2>/dev/null ||
  APPIMAGE_EXTRACT_AND_RUN=1 "$out" --version 2>/dev/null ||
  true

 echo "AppImage: $out"
}

# ── Dispatch ───────────────────────────────────────────────────────────────

case "$mode" in
local)
 build_local
 ;;
docker)
 build_docker
 ;;
appimage)
 build_appimage "$tag_arg" "$version"
 ;;
*)
 echo "unknown mode: $mode" >&2
 exit 1
 ;;
esac
