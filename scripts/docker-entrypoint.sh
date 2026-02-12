#!/usr/bin/env bash
set -euo pipefail

RUST_CONTROL_PORT="${RUST_CONTROL_PORT:-9091}"
RUST_TCP_PORT="${RUST_TCP_PORT:-4001}"
PY_HOST="${PY_HOST:-0.0.0.0}"
PY_PORT="${PY_PORT:-8000}"

shard-daemon --control-port "${RUST_CONTROL_PORT}" --tcp-port "${RUST_TCP_PORT}" &
DAEMON_PID=$!

cleanup() {
  kill "${DAEMON_PID}" >/dev/null 2>&1 || true
}
trap cleanup EXIT INT TERM

cd /app/desktop/python
python run.py --host "${PY_HOST}" --port "${PY_PORT}" --rust-url "http://127.0.0.1:${RUST_CONTROL_PORT}"
