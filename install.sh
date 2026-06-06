#!/usr/bin/env bash
#
# install.sh — install the `lens` static-analysis CLI on Linux or macOS.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/fatmuh/lens/main/install.sh | sh
#
# Environment variables (all optional):
#   LENS_VERSION       — release tag, e.g. v0.1.0.  Default: latest.
#   LENS_REPO          — GitHub owner/repo.         Default: fatmuh/lens.
#   LENS_INSTALL_DIR   — destination directory.     Default: /usr/local/bin.

set -euo pipefail

REPO="${LENS_REPO:-fatmuh/lens}"
VERSION="${LENS_VERSION:-latest}"
INSTALL_DIR="${LENS_INSTALL_DIR:-/usr/local/bin}"

# --- Detect platform -------------------------------------------------------

uname_s=$(uname -s)
uname_m=$(uname -m)

case "${uname_s}-${uname_m}" in
  Linux-x86_64)   TARGET="x86_64-unknown-linux-gnu" ;;
  Linux-aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
  Darwin-x86_64) TARGET="x86_64-apple-darwin" ;;
  Darwin-arm64)  TARGET="aarch64-apple-darwin" ;;
  *) echo "Unsupported platform: ${uname_s}-${uname_m}" >&2
     echo "Please open an issue at https://github.com/${REPO}/issues" >&2
     exit 1 ;;
esac

# --- Build download URL ----------------------------------------------------

if [ "${VERSION}" = "latest" ]; then
  URL="https://github.com/${REPO}/releases/latest/download/lens-${TARGET}.tar.gz"
else
  URL="https://github.com/${REPO}/releases/download/${VERSION}/lens-${TARGET}.tar.gz"
fi

echo "Installing lens ${VERSION} for ${TARGET}..."

# --- Download & extract ----------------------------------------------------

TMP=$(mktemp -d)
trap 'rm -rf "${TMP}"' EXIT

if command -v curl >/dev/null 2>&1; then
  curl -fsSL "${URL}" -o "${TMP}/lens.tar.gz"
elif command -v wget >/dev/null 2>&1; then
  wget -q "${URL}" -O "${TMP}/lens.tar.gz"
else
  echo "Neither curl nor wget is available. Please install one and retry." >&2
  exit 1
fi

tar -xzf "${TMP}/lens.tar.gz" -C "${TMP}"

if [ ! -f "${TMP}/lens" ]; then
  echo "Downloaded archive did not contain a 'lens' binary." >&2
  exit 1
fi

# --- Install ---------------------------------------------------------------

if [ -w "${INSTALL_DIR}" ]; then
  install -m 0755 "${TMP}/lens" "${INSTALL_DIR}/lens"
else
  echo "Need sudo to install to ${INSTALL_DIR}..."
  sudo install -m 0755 "${TMP}/lens" "${INSTALL_DIR}/lens"
fi

echo
echo "✓ lens installed to ${INSTALL_DIR}/lens"
echo
"${INSTALL_DIR}/lens" --version
