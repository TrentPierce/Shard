# Shard: Hybrid Distributed Inference Network

Shard now includes **Phase-5 production hardening scaffolds** for binary sealing and release packaging.

## Core Components

- **Python Driver API** (`desktop/python/oracle_api.py`)
  - OpenAI-compatible chat endpoint.
  - Topology endpoint: `GET /v1/system/topology`.
- **Rust Sidecar** (`desktop/rust/src/main.rs`)
  - WebRTC-direct listen + handshake.
  - Gossipsub work topics: `shard-work`, `shard-work-result`.
- **C++ Shard Engine hard shim** (`cpp/shard-bridge/`)
  - Stable guaranteed C-ABI (`shard_init`, `shard_eval`, `shard_get_logits`, `shard_rollback`).
- **Browser Scout** (`web/`)
  - Auto-fetch topology from localhost.
  - Service worker startup local-oracle detection gate.

## Phase-5 Additions

1. **Hard Shim ABI**
   - Added `cpp/shard-bridge/shard_bridge.cpp` + `include/shard_bridge.h`.
   - Added CMake build to produce `shard_engine.dll` / `libshard_engine.so`.
2. **Unified release forge**
   - Added `scripts/build_release.py` to orchestrate Rust + C++ + Python packaging.
   - Supports `--mock` mode for CI environments without full toolchains.
3. **Release gauntlet tests**
   - Added `tests/release_test.py` to validate bundle layout, engine symbol availability, and double-dip policy hooks.
4. **Dist layout**
   - Produces `dist/ShardAI/` with bundled API app and `_internal/` daemon/engine/web assets.

## Constraints Preserved

- No central inference server.
- Browser-to-desktop transport through libp2p WebRTC-direct.
- Local auto-discovery for seamless first-hop bridge.
