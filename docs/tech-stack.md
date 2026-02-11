# Tech Stack Selection (Final Confirmation)

## Networking

- **Selected:** libp2p across all runtimes.
- **Browser:** `js-libp2p` + `@libp2p/webrtc` + Noise.
- **Desktop:** `rust-libp2p` daemon with `webrtc-direct` listener + Kademlia + Gossipsub + request/response.
- **PubSub topics:**
  - `shard-work` for speculative work auction.
  - `shard-work-result` for scout draft token submissions.

## Model Runtime

- **Oracle engine ABI:** `cpp/shard-bridge` (`shard_engine.dll` / `libshard_engine.so`) with strict C exports.
- **Python bridge:** ctypes calls only the guaranteed `shard_*` symbols (no optional symbol probing).
- **Scout (Browser):** `@mlc-ai/web-llm` for draft tokens.

## Control Plane

- **Python API:** FastAPI (`/v1/chat/completions`, `/v1/system/topology`).
- **Python ↔ Rust:** gRPC over UDS (`desktop/control_plane/shard_control.proto`).
- **Local auto-discovery:** Rust publishes WebRTC multiaddr, Python exposes it to browser.

## Release Engineering

- **Build orchestrator:** `scripts/build_release.py`.
- **Bundle output:** `dist/ShardAI/` with API app + daemon + shard engine + web assets.
- **Binary validation:** `tests/release_test.py`.

## Explicit Non-Selection

- **Not selected:** PeerJS.
- **Reason:** Missing Kademlia DHT + protocol control needed for trust-minimized speculative auctions.
