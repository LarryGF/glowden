# Build environment for the .deb: the oldest supported base (Ubuntu 22.04, glibc 2.35), so the
# package runs on Ubuntu 22.04+ and Debian 12+.
FROM ubuntu:22.04
ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get install -y --no-install-recommends \
      build-essential ca-certificates curl file pkg-config xz-utils \
      libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
      libpipewire-0.3-dev libsystemd-dev libdbus-1-dev libssl-dev \
    && rm -rf /var/lib/apt/lists/*
ENV RUSTUP_HOME=/opt/rustup CARGO_HOME=/opt/cargo PATH=/opt/cargo/bin:/opt/node/bin:$PATH
RUN curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain stable
RUN mkdir /opt/node && curl -sSfL https://nodejs.org/dist/v22.20.0/node-v22.20.0-linux-x64.tar.xz \
      | tar -xJ --strip-components=1 -C /opt/node \
    && npm install -g pnpm@12
WORKDIR /build
