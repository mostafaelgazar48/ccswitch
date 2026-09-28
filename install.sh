#!/bin/sh
# Install ccswitch from GitHub Releases (Linux and macOS).
#
#   curl -fsSL https://raw.githubusercontent.com/mostafaelgazar48/ccswitch/main/install.sh | sh
#
# Options (environment variables):
#   CCSWITCH_VERSION      Release tag to install, e.g. v0.1.0 (default: latest)
#   CCSWITCH_INSTALL_DIR  Where to put the binary (default: ~/.local/bin)
set -eu

REPO="mostafaelgazar48/ccswitch"
VERSION="${CCSWITCH_VERSION:-latest}"
BIN_DIR="${CCSWITCH_INSTALL_DIR:-$HOME/.local/bin}"

fail() {
    echo "ccswitch install: $*" >&2
    exit 1
}

case "$(uname -s)-$(uname -m)" in
    Linux-x86_64 | Linux-amd64) target=x86_64-unknown-linux-musl ;;
    Linux-aarch64 | Linux-arm64) target=aarch64-unknown-linux-musl ;;
    Darwin-x86_64) target=x86_64-apple-darwin ;;
    Darwin-arm64) target=aarch64-apple-darwin ;;
    *) fail "no prebuilt binary for $(uname -s) $(uname -m); build from source: cargo install --git https://github.com/$REPO" ;;
esac

if [ "$VERSION" = "latest" ]; then
    base="https://github.com/$REPO/releases/latest/download"
else
    base="https://github.com/$REPO/releases/download/$VERSION"
fi
asset="ccswitch-$target.tar.gz"

download() {
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "$1" -o "$2"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$2" "$1"
    else
        fail "curl or wget is required"
    fi
}

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d' ' -f1
    else
        shasum -a 256 "$1" | cut -d' ' -f1
    fi
}

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "Downloading $asset ($VERSION)..."
download "$base/$asset" "$tmp/$asset" || fail "download failed: $base/$asset"
download "$base/$asset.sha256" "$tmp/$asset.sha256" || fail "checksum download failed"

expected=$(cut -d' ' -f1 <"$tmp/$asset.sha256")
[ "$expected" = "$(sha256 "$tmp/$asset")" ] || fail "checksum mismatch for $asset"

tar xzf "$tmp/$asset" -C "$tmp" ccswitch
mkdir -p "$BIN_DIR"
mv "$tmp/ccswitch" "$BIN_DIR/ccswitch"
chmod 755 "$BIN_DIR/ccswitch"

echo "Installed $("$BIN_DIR/ccswitch" --version) to $BIN_DIR/ccswitch"
case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) echo "Note: $BIN_DIR is not on your PATH. Add this to your shell profile:
  export PATH=\"$BIN_DIR:\$PATH\"" ;;
esac
