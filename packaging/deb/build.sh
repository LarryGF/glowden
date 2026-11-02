#!/usr/bin/env bash
# Builds dist/glowden_<version>_amd64.deb in an Ubuntu 22.04 container from the committed tree
# (git HEAD), so the result runs on Ubuntu 22.04+ and Debian 12+.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
image=glowden-deb-build
docker build -t "$image" "$root/packaging/deb"
mkdir -p "$root/dist"
git -C "$root" archive --format=tar HEAD | docker run --rm -i \
  -v glowden-deb-cargo:/opt/cargo/registry -v glowden-deb-target:/build/target \
  -v "$root/dist:/out" -e HOST_UID="$(id -u)" -e HOST_GID="$(id -g)" "$image" bash -c '
    set -euo pipefail
    tar -x -C /build
    cd /build
    cargo build --release --locked -p glowdend -p glowden-cli -p glowden-voice -p glowden-sampler
    cd app
    pnpm install --frozen-lockfile
    pnpm tauri build --bundles deb --config ../packaging/deb/tauri.deb.json
    deb=$(ls /build/target/release/bundle/deb/*.deb)
    version=$(sed -n "s/^version = \"\(.*\)\"/\1/p" /build/Cargo.toml | head -1)
    cp "$deb" "/out/glowden_${version}_amd64.deb"
    chown "$HOST_UID:$HOST_GID" "/out/glowden_${version}_amd64.deb"
  '
ls -l "$root"/dist/*.deb
