FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libgtk-4-dev \
    libvte-2.91-gtk4-dev \
    meson \
    ninja-build \
    libwayland-dev \
    wayland-protocols \
    libgirepository1.0-dev \
    zstd \
    git \
    curl \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN git clone --depth 1 --branch v1.3.0 \
      https://github.com/wmww/gtk4-layer-shell.git /tmp/gtk4-layer-shell \
    && meson setup /tmp/gtk4-layer-shell/build /tmp/gtk4-layer-shell \
      -Dexamples=false \
      -Ddocs=false \
      -Dtests=false \
      -Dintrospection=false \
      -Dvapi=false \
    && ninja -C /tmp/gtk4-layer-shell/build \
    && ninja -C /tmp/gtk4-layer-shell/build install \
    && ldconfig \
    && rm -rf /tmp/gtk4-layer-shell

ENV LIBRARY_PATH=/usr/local/lib
