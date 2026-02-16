#!/usr/bin/env bash
set -euo pipefail

API_URL="${1:-http://127.0.0.1:8000}"
RUST_URL="${2:-http://127.0.0.1:9091}"

echo "[health] checking API at ${API_URL}/health"
curl --fail --silent --show-error "${API_URL}/health" >/dev/null

echo "[health] checking Rust daemon at ${RUST_URL}/health"
curl --fail --silent --show-error "${RUST_URL}/health" >/dev/null

echo "[health] checking metrics endpoint"
curl --fail --silent --show-error "${API_URL}/metrics" >/dev/null

echo "[health] all checks passed"
