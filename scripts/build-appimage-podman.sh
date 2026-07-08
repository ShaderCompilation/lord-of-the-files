#!/usr/bin/env bash
set -euo pipefail

IMAGE="${IMAGE:-ubuntu:22.04}"
PNPM_VERSION="${PNPM_VERSION:-11.8.0}"
NODE_MAJOR="${NODE_MAJOR:-22}"

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

if ! command -v podman >/dev/null 2>&1; then
  echo "error: podman is required to build the AppImage container" >&2
  exit 1
fi

echo "Building Linux AppImage in Podman image ${IMAGE}"
echo "Output: ${REPO_ROOT}/src-tauri/target/release/bundle/appimage"

podman run --rm -it \
  --volume "${REPO_ROOT}:/work:Z" \
  --workdir /work \
  --env CI=true \
  --env PNPM_HOME=/root/.local/share/pnpm \
  --env XDG_CACHE_HOME=/work/.tauri-cache \
  "${IMAGE}" \
  bash -lc "
    set -euo pipefail

    export DEBIAN_FRONTEND=noninteractive
    apt-get update
    apt-get install -y --no-install-recommends \
      ca-certificates \
      curl \
      build-essential \
      pkg-config \
      libwebkit2gtk-4.1-dev \
      libayatana-appindicator3-dev \
      librsvg2-dev \
      patchelf \
      libssl-dev \
      libxdo-dev \
      libdbus-1-dev \
      file \
      wget

    cache_dir=\"\${XDG_CACHE_HOME}/tauri\"
    mkdir -p \"\${cache_dir}\"

    download_tauri_helper() {
      local url=\"\$1\"
      local dest=\"\${cache_dir}/\$2\"

      if [ -s \"\${dest}\" ]; then
        echo \"Using cached \${dest}\"
        return
      fi

      echo \"Downloading \${url}\"
      curl -fL --retry 8 --retry-all-errors --retry-delay 10 --connect-timeout 30 \
        -o \"\${dest}.tmp\" \"\${url}\"
      chmod +x \"\${dest}.tmp\"
      mv \"\${dest}.tmp\" \"\${dest}\"
    }

    download_tauri_helper https://github.com/tauri-apps/binary-releases/releases/download/apprun-old/AppRun-x86_64 AppRun-x86_64
    download_tauri_helper https://github.com/tauri-apps/binary-releases/releases/download/linuxdeploy/linuxdeploy-x86_64.AppImage linuxdeploy-x86_64.AppImage
    download_tauri_helper https://raw.githubusercontent.com/tauri-apps/linuxdeploy-plugin-gtk/master/linuxdeploy-plugin-gtk.sh linuxdeploy-plugin-gtk.sh
    download_tauri_helper https://raw.githubusercontent.com/tauri-apps/linuxdeploy-plugin-gstreamer/master/linuxdeploy-plugin-gstreamer.sh linuxdeploy-plugin-gstreamer.sh

    curl -fsSL https://sh.rustup.rs | sh -s -- -y --profile minimal
    . /root/.cargo/env

    curl -fsSL https://deb.nodesource.com/setup_${NODE_MAJOR}.x | bash -
    apt-get install -y --no-install-recommends nodejs
    node -e 'const [major, minor] = process.versions.node.split(\`.\`).map(Number); if (major < 22 || (major === 22 && minor < 13)) { console.error(\`error: pnpm ${PNPM_VERSION} requires Node.js >=22.13. Current Node.js is \${process.versions.node}. Set NODE_MAJOR=22 or use an older PNPM_VERSION.\`); process.exit(1); }'
    corepack enable
    corepack prepare pnpm@${PNPM_VERSION} --activate

    pnpm install --frozen-lockfile
    pnpm tauri build --bundles appimage
  "
