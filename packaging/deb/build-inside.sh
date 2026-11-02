#!/usr/bin/env bash
# Builds the .deb from the current tree into OUT (default dist/). Runs on Ubuntu 22.04: in
# the container from build.sh, or directly on CI's ubuntu-22.04 runner.
set -euo pipefail
root=$(cd "$(dirname "$0")/../.." && pwd)
out=$(realpath -m "${1:-$root/dist}")
cd "$root"
cargo build --release --locked -p glowdend -p glowden-cli -p glowden-sampler
cd app
pnpm install --frozen-lockfile
pnpm tauri build --bundles deb --config ../packaging/deb/tauri.deb.json
version=$(sed -n 's/^version = "\(.*\)".*/\1/p' "$root/Cargo.toml" | head -1)
mkdir -p "$out"
cp "$root"/target/release/bundle/deb/*.deb "$out/glowden_${version}_amd64.deb"
echo "$out/glowden_${version}_amd64.deb"
