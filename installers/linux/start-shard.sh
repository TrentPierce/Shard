#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

BIN="${SCRIPT_DIR}/shard-daemon-x86_64-unknown-linux-gnu"
if [[ ! -f "${BIN}" ]]; then
  BIN="${SCRIPT_DIR}/shard-daemon"
fi

if [[ ! -f "${BIN}" ]]; then
  echo "[ERROR] Could not find shard daemon binary in ${SCRIPT_DIR}" >&2
  echo "Expected shard-daemon-x86_64-unknown-linux-gnu or shard-daemon" >&2
  exit 1
fi

chmod +x "${BIN}"
TCP_PORT="${SHARD_TCP_PORT:-4001}"

echo "Starting Shard daemon with TCP port ${TCP_PORT}..."
echo "Binary: ${BIN}"
echo

exec "${BIN}" --tcp-port "${TCP_PORT}"
