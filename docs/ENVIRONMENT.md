# Environment Variables

Shard uses environment variables to configure all components. Below is the **single source of truth** for all variables.

---

## Quick Start

Copy the example file:
```bash
cp .env.example .env
```

---

## Core Variables

### API Server (`desktop/python/`)

| Variable | Default | Description |
|----------|---------|-------------|
| `SHARD_HOST` | `0.0.0.0` | Bind address for the Shard API |
| `SHARD_PORT` | `8000` | Port for the Shard API |
| `SHARD_RUST_SIDECAR_URL` | `http://127.0.0.1:9091` | URL of the Rust P2P daemon |
| `SHARD_LOG_LEVEL` | `INFO` | Logging verbosity: `DEBUG`, `INFO`, `WARNING`, `ERROR` |

### Model & Inference

| Variable | Default | Description |
|----------|---------|-------------|
| `BITNET_LIB` | *(required)* | Path to the compiled BitNet shared library (`.dll`, `.so`, `.dylib`) |
| `BITNET_MODEL` | *(required)* | Path to the GGUF model file |
| `SHARD_N_THREADS` | `4` | Number of CPU threads for inference |
| `SHARD_CTX_SIZE` | `2048` | Context window size in tokens |
| `SHARD_BATCH_SIZE` | `32` | Batch size for token generation |
| `SHARD_PROMPT_FORMAT` | `auto` | Prompt format: `auto`, `llama3`, `plain` |
| `SHARD_MAX_TOKENS` | `512` | Maximum tokens per chat response |

### Security & Rate Limiting

| Variable | Default | Description |
|----------|---------|-------------|
| `SHARD_API_KEYS` | *(empty)* | Comma-separated API keys. If empty, auth is disabled |
| `SHARD_RATE_LIMIT_PER_MINUTE` | `30` | Max requests per client per minute |
| `SHARD_MAX_PROMPT_CHARS` | `10000` | Maximum prompt length in characters |

### CORS & Networking

| Variable | Default | Description |
|----------|---------|-------------|
| `SHARD_CORS_ORIGINS` | `*` | Comma-separated allowed CORS origins |
| `SHARD_CORS_CREDENTIALS` | `true` | Allow credentials in CORS requests |

### Verification & Golden Tickets

| Variable | Default | Description |
|----------|---------|-------------|
| `SHARD_GOLDEN_TICKET_RATE` | `0.05` | Probability of injecting a verification prompt (0.0–1.0) |
| `SHARD_MIN_SCOUT_ACCURACY` | `0.8` | Minimum accuracy before a Scout is penalized |
| `SHARD_BAN_THRESHOLD` | `3` | Number of failed Golden Tickets before ban |

---

## Rust Daemon (`desktop/rust/`)

The daemon is configured via CLI flags, not environment variables:

```bash
shard-daemon \
  --control-port 9091 \
  --tcp-port 4001 \
  --webrtc-port 9090 \
  --quic-port 9092 \
  --bootstrap /ip4/54.224.107.75/tcp/4001/p2p/12D3KooW...
```

| Flag | Default | Description |
|------|---------|-------------|
| `--control-port` | `9091` | HTTP API port for the control plane |
| `--tcp-port` | `4001` | TCP transport listen port |
| `--webrtc-port` | `9090` | WebRTC-direct UDP port |
| `--quic-port` | `9092` | QUIC transport UDP port |
| `--bootstrap` | *(empty)* | Bootstrap peer multiaddr |
| `--bootstrap-file` | *(empty)* | File containing bootstrap peers (one per line) |

---

## Web Client (`web/`)

| Variable | Default | Description |
|----------|---------|-------------|
| `NEXT_PUBLIC_API_URL` | `/api` | API base URL (proxied through Vercel) |
| `NEXT_PUBLIC_WS_URL` | *(auto)* | WebSocket URL for real-time updates |
| `NEXT_OUTPUT_MODE` | *(unset)* | Set to `export` for static site generation |

---

## Docker Compose

Docker Compose uses the variables above plus:

| Variable | Default | Description |
|----------|---------|-------------|
| `SHARD_INFERENCE_PORT` | `7000` | Port for the inference engine container |
| `GRAFANA_PORT` | `3001` | Grafana dashboard port (monitoring profile) |
| `PROMETHEUS_PORT` | `9095` | Prometheus metrics port (monitoring profile) |
