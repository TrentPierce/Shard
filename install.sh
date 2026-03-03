#!/bin/bash
set -euo pipefail

# Shard Daemon Installer
# Usage: curl -sSL https://shard.network/install.sh | bash

INSTALL_DIR="/usr/local/bin"
BINARY_NAME="shard-daemon"

echo "==> Detecting platform..."
VERSION=$(curl -sfS https://api.github.com/repos/TrentPierce/Shard/releases/latest | grep tag_name | cut -d'"' -f4)
if [ -z "$VERSION" ]; then
  echo "Error: Could not determine latest release version." >&2
  exit 1
fi

OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m)
TARBALL="shard-daemon-${OS}-${ARCH}.tar.gz"
BASE_URL="https://github.com/TrentPierce/Shard/releases/download/${VERSION}"

echo "==> Downloading ${BINARY_NAME} ${VERSION} for ${OS}/${ARCH}..."
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT

if ! curl -fSL -o "${TMPDIR}/${TARBALL}" "${BASE_URL}/${TARBALL}"; then
  echo "Error: Failed to download ${TARBALL}." >&2
  echo "Check that a release exists for your platform at:" >&2
  echo "  ${BASE_URL}/${TARBALL}" >&2
  exit 1
fi

# Verify checksum if .sha256 sidecar is available
if curl -fsSL -o "${TMPDIR}/${TARBALL}.sha256" "${BASE_URL}/${TARBALL}.sha256" 2>/dev/null; then
  echo "==> Verifying checksum..."
  (cd "$TMPDIR" && sha256sum -c "${TARBALL}.sha256")
else
  echo "Warning: No .sha256 checksum found; skipping verification."
fi

echo "==> Extracting..."
if ! tar xzf "${TMPDIR}/${TARBALL}" -C "${TMPDIR}"; then
  echo "Error: Failed to extract ${TARBALL}." >&2
  exit 1
fi

echo ""
echo "This will install ${BINARY_NAME} to ${INSTALL_DIR}/${BINARY_NAME}."
read -rp "Proceed with sudo install? [y/N] " confirm
if [[ ! "$confirm" =~ ^[Yy]$ ]]; then
  echo "Aborted. Binary extracted to: ${TMPDIR}/${BINARY_NAME}"
  trap - EXIT  # keep tmpdir so user can install manually
  exit 0
fi

sudo mv "${TMPDIR}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
echo ""
echo "==> Installed ${BINARY_NAME} ${VERSION} to ${INSTALL_DIR}/${BINARY_NAME}"
echo ""
echo "To start contributing:"
echo "  shard-daemon --contribute"
echo ""
echo "To check health:"
echo "  curl http://localhost:9091/health"
