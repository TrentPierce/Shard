# Shard: Hybrid Distributed Inference Network

Shard now includes **Phase-5 production hardening scaffolds** focused on binary sealing.

## What is hardened

- **C++ Hard Shim (`cpp/shard-bridge/`)**
  - Exposes a strict C ABI (`shard_init`, `shard_eval`, `shard_get_logits`, `shard_rollback`, etc.).
  - Intended to statically link BitNet internals and remove runtime symbol ambiguity.
- **Python bridge (`desktop/python/bitnet/ctypes_bridge.py`)**
  - Fail-fast mandatory symbol binding to `shard_engine` ABI.
  - No optional-symbol probing path.
- **Speculative loop (`desktop/python/inference.py`)**
  - Fuzzy top-k verification and sequence-aware auction acceptance.
- **Release forge (`scripts/build_release.py`)**
  - One script for Rust release binary, C++ engine build, and Python freezing.
- **Release gauntlet (`tests/release_test.py`)**
  - Binary-level smoke checks for packaged artifacts.

## Build outputs

- `build/bin/shard-daemon(.exe)`
- `build/lib/libshard_engine.so` or `build/lib/shard_engine.dll`
- `dist/ShardAI/` (PyInstaller one-dir bundle) or `dist/raw/` for skip-freeze mode

## Constraints preserved

- No central inference server.
- Browser-to-desktop bridge over libp2p/WebRTC-direct.
- Double-dip protection at service-worker startup.
