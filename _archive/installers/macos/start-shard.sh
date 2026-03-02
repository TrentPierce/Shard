#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

BIN="${SCRIPT_DIR}/shard-daemon-aarch64-apple-darwin"
if [[ ! -f "${BIN}" ]]; then
  BIN="${SCRIPT_DIR}/shard-daemon-x86_64-apple-darwin"
fi
if [[ ! -f "${BIN}" ]]; then
  BIN="${SCRIPT_DIR}/shard-daemon"
fi

if [[ ! -f "${BIN}" ]]; then
  echo "[ERROR] Could not find shard daemon binary in ${SCRIPT_DIR}" >&2
  echo "Expected shard-daemon-aarch64-apple-darwin, shard-daemon-x86_64-apple-darwin, or shard-daemon" >&2
  exit 1
fi

chmod +x "${BIN}"
TCP_PORT="${SHARD_TCP_PORT:-4001}"
CONTROL_PORT="${SHARD_CONTROL_PORT:-9091}"
WEBRTC_PORT="${SHARD_WEBRTC_PORT:-9090}"
QUIC_PORT="${SHARD_QUIC_PORT:-9092}"

echo "Starting Shard daemon with TCP port ${TCP_PORT}..."
echo "Binary: ${BIN}"
echo

BOOTSTRAP_ARGS=()
if [[ -n "${SHARD_BOOTSTRAP_NODES:-}" ]]; then
  while IFS= read -r node; do
    [[ -n "${node}" ]] && BOOTSTRAP_ARGS+=("--bootstrap-node" "${node}")
  done < <(printf "%s" "${SHARD_BOOTSTRAP_NODES}" | tr ",;" "\n")
fi

exec "${BIN}" --tcp-port "${TCP_PORT}" --control-port "${CONTROL_PORT}" --webrtc-port "${WEBRTC_PORT}" --quic-port "${QUIC_PORT}" "${BOOTSTRAP_ARGS[@]}"
