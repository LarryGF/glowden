#!/usr/bin/env bash
# Builds dist/lhub_<version>_amd64.deb in an Ubuntu 22.04 container from the committed tree
# (git HEAD), so the result runs on Ubuntu 22.04+ and Debian 12+.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
image=lhub-deb-build
docker build -t "$image" "$root/packaging/deb"
mkdir -p "$root/dist"
git -C "$root" archive --format=tar HEAD | docker run --rm -i \
  -v lhub-deb-cargo:/opt/cargo/registry -v lhub-deb-target:/build/target \
  -v "$root/dist:/out" -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" "$image" bash -c '
    set -euo pipefail
    tar -x -C /build
    cd /build
    cargo build --release --locked -p lhubd -p lhub-cli -p lhub-voice -p lhub-sampler
    cd app
    pnpm install --frozen-lockfile
    pnpm tauri build --bundles deb --config ../packaging/deb/tauri.deb.json
    cp /build/target/release/bundle/deb/*.deb /out/
    chown "$HOST_UID:$HOST_GID" /out/*.deb
  '
ls -l "$root"/dist/*.deb
