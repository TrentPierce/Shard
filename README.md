# Shard: Hybrid Distributed Inference Network

Shard now supports a **Phase-3 speculative auction scaffold**:

- Browser Scouts can receive work context and return draft tokens.
- Rust sidecar can publish work to gossipsub and forward results back to Python.
- Python API exposes topology so browser auto-discovers local WebRTC Oracle address.

## Core Components

- **Python Driver API** (`desktop/python/oracle_api.py`)
  - OpenAI-compatible chat endpoint.
  - Topology endpoint: `GET /v1/system/topology`.
- **Rust Sidecar** (`desktop/rust/src/main.rs`)
  - WebRTC-direct listen + handshake.
  - Gossipsub work topics: `shard-work`, `shard-work-result`.
- **Browser Scout** (`web/`)
  - Auto-fetch topology from localhost.
  - Service worker scaffold for work consumption and result publishing.

## Phase-3 Additions

1. **Auto-Discovery**
   - Rust writes local WebRTC multiaddr hint.
   - Python serves topology for browser bootstrap.
2. **Thought Protocol**
   - Added `WorkRequest`/`WorkResponse` to control-plane proto.
3. **Speculative Loop Scaffold**
   - Added `desktop/python/inference.py` cooperative generation loop with 50ms work checks.

## Constraints Preserved

- No central inference server.
- Double-dip protection path for local desktop oracle.
- Browser-to-desktop transport through libp2p WebRTC-direct.
