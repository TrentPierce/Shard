#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "[1/3] Building shard-daemon and shard-load-test"
cd "$ROOT_DIR/desktop/rust"
cargo build --release

echo "[2/3] Installing web dependencies"
cd "$ROOT_DIR/web"
npm ci

echo "[3/3] Quickstart checks"
cd "$ROOT_DIR"
echo "Run daemon: docker compose up --build shard-daemon"
echo "Open dashboard: http://127.0.0.1:9091/dashboard"
echo "Run load test: $ROOT_DIR/desktop/rust/target/release/shard-load-test --requests 100 --concurrency 100"
