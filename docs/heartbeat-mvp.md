# Heartbeat + Auto-Discovery MVP

## Goal

Prove browser-to-desktop bridge with:
1. Auto-discovery of local Oracle WebRTC multiaddr.
2. PING/PONG heartbeat over `/shard/1.0.0/handshake`.

## Auto-Discovery Flow

1. Rust daemon starts and writes full WebRTC-direct multiaddr (with `/certhash/...`) to local topology hint.
2. Python API exposes it at `GET /v1/system/topology`.
3. Browser fetches topology and dials immediately (no manual copy/paste).

## Heartbeat Test

- Open web app.
- Verify Known Oracle Multiaddr is populated from topology endpoint.
- Click **Send PING**.
- Expect `PONG rtt=...ms` in browser and latency log in Rust daemon.

## Next

- Replace topology hint file bridge with direct gRPC `UpdateTopology` callback.
- Replace worker placeholder draft generation with real WebLLM token generation.
