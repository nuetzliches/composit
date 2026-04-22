#!/usr/bin/env sh
# composit installer — downloads the pre-built binary for the current platform.
# Usage: curl -fsSL https://nuetzliches.github.io/composit/install.sh | sh
set -eu

REPO="nuetzliches/composit"
INSTALL_DIR="${COMPOSIT_INSTALL_DIR:-$HOME/.local/bin}"

die() { echo "composit-install: $*" >&2; exit 1; }

# Detect platform
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin) OS="apple-darwin" ;;
  Linux)  OS="unknown-linux-musl" ;;
  *)      die "unsupported OS: $OS" ;;
esac

case "$ARCH" in
  x86_64|amd64) ARCH="x86_64" ;;
  arm64|aarch64) ARCH="aarch64" ;;
  *)             die "unsupported arch: $ARCH" ;;
esac

TARGET="${ARCH}-${OS}"

# Resolve latest release
if command -v curl > /dev/null 2>&1; then
  FETCH="curl -fsSL"
elif command -v wget > /dev/null 2>&1; then
  FETCH="wget -qO-"
else
  die "neither curl nor wget found"
fi

LATEST=$($FETCH "https://api.github.com/repos/${REPO}/releases/latest" \
  | grep '"tag_name"' | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')

[ -n "$LATEST" ] || die "could not determine latest release"

ARCHIVE="composit-${TARGET}.tar.gz"
URL="https://github.com/${REPO}/releases/download/${LATEST}/${ARCHIVE}"

# Download and install
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

echo "composit-install: downloading ${LATEST} for ${TARGET}..."
$FETCH "$URL" > "${TMP}/${ARCHIVE}"
tar -xzf "${TMP}/${ARCHIVE}" -C "$TMP"

mkdir -p "$INSTALL_DIR"
install -m755 "${TMP}/composit" "${INSTALL_DIR}/composit"

echo "composit-install: installed to ${INSTALL_DIR}/composit"

# Check PATH
case ":$PATH:" in
  *":${INSTALL_DIR}:"*) ;;
  *) echo "composit-install: add '${INSTALL_DIR}' to your PATH" ;;
esac
