#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ARTIFACT_DIR="${ROOT_DIR}/.offline"

mkdir -p "${ARTIFACT_DIR}/python-wheels" "${ARTIFACT_DIR}/npm-cache" "${ARTIFACT_DIR}/cargo-vendor"

echo "[1/3] Building Python wheelhouse"
python -m pip wheel -r "${ROOT_DIR}/desktop/python/requirements.txt" -w "${ARTIFACT_DIR}/python-wheels"

echo "[2/3] Priming npm cache"
cd "${ROOT_DIR}/web"
npm ci --cache "${ARTIFACT_DIR}/npm-cache" --prefer-offline || true

echo "[3/3] Vendoring Rust crates"
cd "${ROOT_DIR}/desktop/rust"
cargo vendor "${ARTIFACT_DIR}/cargo-vendor"

echo "Offline artifacts prepared under ${ARTIFACT_DIR}"
