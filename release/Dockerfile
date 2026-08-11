# Dockerfile for building Codex with FMSH license.
#
# Build (pass proxy so the builder can reach the network):
#   docker build \
#     --build-arg HTTP_PROXY="$HTTP_PROXY" \
#     --build-arg HTTPS_PROXY="$HTTPS_PROXY" \
#     --build-arg NO_PROXY="$NO_PROXY" \
#     --build-arg UBUNTU_VERSION=20.04 \
#     -t codex:ubuntu-20.04 -f release/Dockerfile .
#
# For Ubuntu 22.04+:
#   --build-arg UBUNTU_VERSION=22.04  -t codex:ubuntu-22.04
#
# Extract the binary:
#   id=$(docker create codex:ubuntu-20.04)
#   docker cp "$id:/usr/local/bin/codex" ./codex
#   docker rm "$id"
#
# Bazel disk/repo caches are persisted across builds via BuildKit cache
# mounts, so incremental rebuilds skip already-compiled actions (V8 etc.).
# The output base is intentionally NOT cached — it holds Bazel server lock
# files that would corrupt across container teardowns.
#
# Note: the apt mirror sed assumes the classic sources.list format.
# Ubuntu 24.04+ uses deb822 ( /etc/apt/sources.list.d/ubuntu.sources );
# adapt the sed or remove it if building on newer bases.

ARG UBUNTU_VERSION=20.04

# ── builder stage ─────────────────────────────────────────────────────────
FROM ubuntu:${UBUNTU_VERSION} AS builder

ARG HTTP_PROXY
ARG HTTPS_PROXY
ARG NO_PROXY
ARG FM_BUILD_SUFFIX=fm.r0-unknown
ENV http_proxy=${HTTP_PROXY} \
    https_proxy=${HTTPS_PROXY} \
    no_proxy=${NO_PROXY} \
    FM_BUILD_SUFFIX=${FM_BUILD_SUFFIX} \
    DEBIAN_FRONTEND=noninteractive

# Aliyun apt mirror for China network (classic sources.list format)
RUN sed -i 's|http://archive.ubuntu.com/ubuntu|http://mirrors.aliyun.com/ubuntu|g' \
        /etc/apt/sources.list \
    && sed -i 's|http://security.ubuntu.com/ubuntu|http://mirrors.aliyun.com/ubuntu|g' \
        /etc/apt/sources.list

# g++ needed for -lstdc++ symlink; libcurl4-openssl-dev for -lcurl symlink
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt/lists,sharing=locked \
    rm -f /etc/apt/apt.conf.d/docker-clean \
    && apt-get update && apt-get install -y --no-install-recommends \
        bash curl git python3 ca-certificates xz-utils \
        g++ libcurl4-openssl-dev
# Create libstdc++.so symlink for -lstdc++ (hermetic LLVM lld needs it)
RUN ln -sf /usr/lib/x86_64-linux-gnu/libstdc++.so.6 /usr/lib/x86_64-linux-gnu/libstdc++.so

# Bazelisk via ghfast.top proxy. BAZELISK_BASE_URL makes bazelisk itself
# download Bazel through the proxy instead of hitting github.com directly.
RUN curl -fsSL "https://ghfast.top/github.com/bazelbuild/bazelisk/releases/download/v1.26.0/bazelisk-linux-amd64" \
        -o /usr/local/bin/bazel \
    && chmod +x /usr/local/bin/bazel
ENV USE_BAZEL_VERSION=9.0.0
ENV BAZELISK_BASE_URL=https://ghfast.top/github.com/bazelbuild/bazel/releases/download

# Redirect ALL git operations on github.com through ghfast.top (covers
# git_repository rules that the BCR proxy doesn't handle).
RUN git config --global url."https://ghfast.top/github.com/".insteadOf "https://github.com/"

WORKDIR /workspace
COPY . .

# Dynamically pre-download all bare github.com archives into distdir.
# This catches BCR module internal downloads (bsdtar, bats-core, toml2json,
# rules_rust, etc.) that bypass the ghfast URL patches and BCR proxy.
RUN bash release/predownload-deps.sh
# cache mounts persist Bazel's disk/repo caches across docker build invocations.
# Do NOT cache the output base (/root/.cache/bazel) — it holds server lock files.
# --nobuild fails fast on any download/patch issue before the long V8 compile.
RUN --mount=type=cache,target=/root/.cache/bazel-disk-cache,sharing=locked \
    --mount=type=cache,target=/root/.cache/bazel-repo-cache,sharing=locked \
    --mount=type=cache,target=/root/.cache/bazel-repo-contents-cache,sharing=locked \
    --mount=type=cache,target=/root/.cache/bazelisk,sharing=locked \
    python3 release/bazel-registry-proxy.py 8765 > /tmp/proxy.log 2>&1 & \
    sleep 1 && \
    bazel build --action_env=FM_BUILD_SUFFIX //codex-rs/cli:codex --nobuild \
    && bazel build --action_env=FM_BUILD_SUFFIX //codex-rs/cli:codex \
    && mkdir -p /codex/lib \
    && cp bazel-bin/codex-rs/cli/codex /codex/codex \
    && strip /codex/codex \
    # Bundle the fmsh-ukey SDK shared libraries next to the binary. The
    # executable's RUNPATH includes $ORIGIN/lib, so the runtime image only
    # needs these files copied to /usr/local/bin/lib. The GM3000 provider
    # (libgm3000.1.0.so) is dlopen'd at runtime via FMSH_UKEY_PROVIDER, so it
    # never appears in NEEDED/_solib_k8 and must be copied from the vendored
    # SDK directory explicitly.
    && cp -L bazel-bin/_solib_k8/*/*.so* /codex/lib/ \
    && gm3000_provider="$(find -L "$(bazel info output_base)/execroot/_main/external" \
        -name 'libgm3000.1.0.so' | head -n1)" \
    && cp -L "$gm3000_provider" /codex/lib/ \
    && bazel shutdown

# ── appimage stage ──────────────────────────────────────────────────────
# Builds the single-file CLI AppImage entirely inside the container:
# assembles AppDir (codex + SDK/closure libs + AppRun), packs squashfs and
# concatenates the type2 runtime. No desktop file / icon (pure CLI).
FROM ubuntu:${UBUNTU_VERSION} AS appimage

ARG HTTP_PROXY
ARG HTTPS_PROXY
ARG NO_PROXY
ARG FM_BUILD_SUFFIX=fm.r0-unknown
ARG FM_BASE_VERSION=0.146.0
ENV http_proxy=${HTTP_PROXY} \
    https_proxy=${HTTPS_PROXY} \
    no_proxy=${NO_PROXY} \
    DEBIAN_FRONTEND=noninteractive

RUN sed -i 's|http://archive.ubuntu.com/ubuntu|http://mirrors.aliyun.com/ubuntu|g' \
        /etc/apt/sources.list \
    && sed -i 's|http://security.ubuntu.com/ubuntu|http://mirrors.aliyun.com/ubuntu|g' \
        /etc/apt/sources.list \
    && apt-get update && apt-get install -y --no-install-recommends \
        curl ca-certificates libcurl4 libstdc++6 libzstd1

COPY --from=builder /codex/codex /app/usr/bin/codex
COPY --from=builder /codex/lib /app/usr/bin/lib/

RUN mkdir -p /app/usr/lib /work \
    && ldd /app/usr/bin/codex \
        | awk -F'=> ' '/=> \// {print $2}' \
        | awk '{print $1}' | sort -u \
        | while read -r lib; do \
            case "$lib" in \
                */ld-linux*|*/libc.so.6|*/libm.so.6|*/libpthread.so.0 \
                |*/libdl.so.2|*/librt.so.1|*/libutil.so.1|*/libresolv.so.2 \
                |*/libfmsh_ukey_sdk.so*|*/libcrypto.so.1.1) continue ;; \
            esac; \
            cp -L "$lib" /app/usr/lib/; \
          done \
    && true

RUN cat > /app/AppRun <<'EOF'
#!/bin/sh
SELF="$(readlink -f "$0")"
APPDIR="${SELF%/*}"
export LD_LIBRARY_PATH="$APPDIR/usr/lib:$APPDIR/usr/bin/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$APPDIR/usr/bin/codex" "$@"
EOF

RUN chmod +x /app/AppRun /app/usr/bin/codex \
    && curl -fsSL "https://ghfast.top/github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage" \
        -o /work/appimagetool \
    && chmod +x /work/appimagetool \
    && curl -fsSL "https://ghfast.top/github.com/AppImage/type2-runtime/releases/download/continuous/runtime-x86_64" \
        -o /work/runtime \
    && cd /work \
    && APPIMAGE_EXTRACT_AND_RUN=1 ./appimagetool --appimage-extract >/dev/null \
    && ./squashfs-root/usr/bin/mksquashfs /app /work/codex.squashfs \
        -comp zstd -b 131072 -noappend >/dev/null \
    && cp /work/runtime "/codex-${FM_BASE_VERSION}-${FM_BUILD_SUFFIX}-x86_64.AppImage" \
    && cat /work/codex.squashfs >> "/codex-${FM_BASE_VERSION}-${FM_BUILD_SUFFIX}-x86_64.AppImage" \
    && chmod +x "/codex-${FM_BASE_VERSION}-${FM_BUILD_SUFFIX}-x86_64.AppImage"

# ── runtime stage ─────────────────────────────────────────────────────────
FROM ubuntu:${UBUNTU_VERSION}

ENV DEBIAN_FRONTEND=noninteractive
RUN --mount=type=cache,target=/var/cache/apt,sharing=locked \
    --mount=type=cache,target=/var/lib/apt/lists,sharing=locked \
    sed -i 's|http://archive.ubuntu.com/ubuntu|http://mirrors.aliyun.com/ubuntu|g' \
        /etc/apt/sources.list \
    && sed -i 's|http://security.ubuntu.com/ubuntu|http://mirrors.aliyun.com/ubuntu|g' \
        /etc/apt/sources.list \
    && rm -f /etc/apt/apt.conf.d/docker-clean \
    && apt-get update && apt-get install -y --no-install-recommends \
        libcurl4 libstdc++6 libzstd1

COPY --from=builder /codex/codex /usr/local/bin/codex
COPY --from=builder /codex/lib /usr/local/bin/lib
RUN chmod +x /usr/local/bin/codex

ENTRYPOINT ["codex"]
