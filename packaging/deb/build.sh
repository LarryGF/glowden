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
    /build/packaging/deb/build-inside.sh /out
    chown "$HOST_UID:$HOST_GID" /out/*.deb
  '
ls -l "$root"/dist/*.deb
