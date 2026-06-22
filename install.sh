#!/usr/bin/env bash
set -euo pipefail

REPO="bryanbill/river"
INSTALL_DIR="${RIVER_INSTALL_DIR:-/usr/local/bin}"

# ---- helpers ----
bold()  { printf "\033[1m%s\033[0m\n" "$*"; }
err()   { printf "\033[31m%s\033[0m\n" "$*" >&2; }
info()  { printf "\033[34m%s\033[0m\n" "$*"; }

# ---- detect platform ----
detect_platform() {
    local os arch
    os=$(uname -s | tr '[:upper:]' '[:lower:]')
    arch=$(uname -m)

    case "$arch" in
        x86_64|amd64) arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *) err "Unsupported architecture: $arch"; exit 1 ;;
    esac

    case "$os" in
        linux)   target="${arch}-unknown-linux-gnu" ;;
        darwin)  target="${arch}-apple-darwin" ;;
        mingw*|msys*|cygwin*|windows_nt) target="x86_64-pc-windows-msvc" ; os="windows" ;;
        *) err "Unsupported OS: $os"; exit 1 ;;
    esac

    echo "$os" "$target"
}

# ---- download and install ----
install_river() {
    local os target
    read -r os target < <(detect_platform)

    local ext
    if [ "$os" = "windows" ]; then
        ext="zip"
        binary="river.exe"
    else
        ext="tar.gz"
        binary="river"
    fi

    local filename="river-${target}.${ext}"
    local release_url="https://github.com/${REPO}/releases/latest/download/${filename}"

    info "Detected  : ${target}"
    info "Downloading: ${release_url}"

    tmpdir=$(mktemp -d)
    trap 'rm -rf "$tmpdir"' EXIT

    if command -v curl > /dev/null 2>&1; then
        curl -fsSL "$release_url" -o "${tmpdir}/${filename}"
    elif command -v wget > /dev/null 2>&1; then
        wget -q "$release_url" -O "${tmpdir}/${filename}"
    else
        err "Need curl or wget to download. Install one and try again."
        exit 1
    fi

    info "Extracting..."

    if [ "$ext" = "tar.gz" ]; then
        tar -xzf "${tmpdir}/${filename}" -C "$tmpdir"
    else
        if ! command -v unzip > /dev/null 2>&1; then
            err "Need unzip on Windows. Install it and try again."
            exit 1
        fi
        unzip -qo "${tmpdir}/${filename}" -d "$tmpdir"
    fi

    local dest="${INSTALL_DIR}/${binary}"
    info "Installing to: ${dest}"

    if [ ! -w "$INSTALL_DIR" ]; then
        sudo install -m 755 "${tmpdir}/${binary}" "$dest"
    else
        install -m 755 "${tmpdir}/${binary}" "$dest"
    fi

    bold "River installed successfully!"
    info "Run:  river --help"
}

install_river
