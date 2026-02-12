# Shard: Hybrid Distributed Inference Network

> **Free, unlimited LLM access** powered by a decentralized P2P inference mesh.
> Contribute compute, earn priority.

---

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│  User  ──→  Python Driver API  ──→  Rust Sidecar (.exe)     │
│              (FastAPI :8000)        (libp2p :9091)            │
│                 │                       │                     │
│                 │  cooperative_generate  │  gossipsub          │
│                 │  SSE streaming        │  shard-work topic   │
│                 ▼                       ▼                     │
│         BitNet Verify            Browser Scouts               │
│         (ctypes bridge)          (WebLLM + libp2p)            │
└──────────────────────────────────────────────────────────────┘
```

### Node Classes

| Class | Name | Hardware | Role |
|-------|------|----------|------|
| A | **Oracle** | Desktop/Server with GPU | Full model host, verifies draft tokens |
| B | **Scout** | Browser (WebGPU) | Runs tiny draft model, submits token guesses |
| C | **Consumer** | Any | Pure consumer, queued behind contributors |

---

## Quick Start

### Prerequisites

- **Rust** (1.75+) — [rustup.rs](https://rustup.rs)
- **Python** (3.11+) — with pip
- **Node.js** (18+) — with npm

### 1. Build the Rust Daemon (the distributable .exe)

```bash
cd desktop/rust
cargo build --release
```

The compiled binary will be at `desktop/rust/target/release/shard-daemon.exe` (Windows) or `shard-daemon` (Linux/Mac).

### 2. Start the Rust Daemon

```bash
# Default: control-plane on :9091, TCP on :4001, WebSocket on :4101
./target/release/shard-daemon

# With custom ports
./target/release/shard-daemon --control-port 9091 --tcp-port 4001

# Connect to a bootstrap peer
./target/release/shard-daemon --bootstrap /ip4/192.168.1.10/tcp/4001

# Use bootstrap list file + periodic reconnection
./target/release/shard-daemon --bootstrap-file ./bootstrap.txt --reconnect-seconds 15

# Debug logging
./target/release/shard-daemon --log-level debug
```

### 3. Start the Python Oracle API

```bash
cd desktop/python
pip install -r requirements.txt

# Default: API on :8000, connects to Rust on :9091
python run.py

# Custom configuration
python run.py --port 8080 --rust-url http://192.168.1.10:9091
```

### 4. Start the Web Client

```bash
cd web
npm install
npm run dev
```

Open [http://localhost:3000](http://localhost:3000) in your browser.

---


### API Security Controls

The Oracle API supports production hardening controls via environment variables:

- `SHARD_API_KEYS` — comma-separated valid API keys. When set, clients must send `Authorization: Bearer <key>` or `X-API-Key: <key>` for `/v1/chat/completions`.
- `SHARD_RATE_LIMIT_PER_MINUTE` — per-client request budget for the chat endpoint (default `60`).
- `SHARD_MAX_PROMPT_CHARS` — hard cap on prompt length in characters (default `16000`).
- `SHARD_LOG_LEVEL` — runtime log level for the Python API (`DEBUG`, `INFO`, `WARNING`, `ERROR`, `CRITICAL`).

## Distribution

### Distributing the Oracle (.exe)

1. Build the release binary:
   ```bash
   cd desktop/rust
   cargo build --release
   ```

2. Ship the binary to other PCs. Each PC runs:
   ```bash
   shard-daemon.exe --bootstrap /ip4/<YOUR_IP>/tcp/4001
   ```

3. On the host PC, also start the Python API:
   ```bash
   cd desktop/python
   python run.py
   ```

### Connecting Peers

On **PC A** (first Oracle):
```bash
shard-daemon.exe
# Note the listen address printed, e.g.:
# oracle listening address: /ip4/192.168.1.10/tcp/4001
```

On **PC B** (second Oracle or Scout):
```bash
shard-daemon.exe --bootstrap /ip4/192.168.1.10/tcp/4001
```

### Browser Scouts

Any browser can join as a Scout by opening the web client. It will:
1. Auto-discover the local Oracle via `/v1/system/topology`
2. Connect via WebSocket/WebRTC
3. Begin generating draft tokens using WebLLM (WebGPU)

---

## Core Components

### Python Driver API (`desktop/python/`)

| File | Purpose |
|------|---------|
| `run.py` | Entry point — starts uvicorn server |
| `oracle_api.py` | FastAPI app with SSE streaming, topology proxy |
| `inference.py` | Cooperative generation loop with work broadcasting |
| `bitnet/ctypes_bridge.py` | In-process model bridge for verification |

**Endpoints:**
- `POST /v1/chat/completions` — OpenAI-compatible (supports `stream: true`)
- `GET /v1/system/topology` — Network topology for browser bootstrap
- `GET /v1/system/peers` — Connected peer list
- `GET /v1/models` — Available models
- `GET /health` — System health

### Rust Sidecar (`desktop/rust/`)

The distributable `.exe` that handles all P2P networking:

- **Transports:** TCP, WebSocket (WebRTC-direct on Linux/Mac)
- **PubSub:** Gossipsub topics `shard-work`, `shard-work-result`
- **Protocols:** `/shard/1.0.0/handshake`, `/shard/oracle/verify/1.0.0`
- **Control API (HTTP :9091):**
  - `GET /health` — Daemon health + peer count
  - `GET /topology` — Listen addresses for browser auto-discovery
  - `GET /peers` — Connected peer details
  - `POST /broadcast-work` — Publish work request to gossipsub
  - `GET /pop-result` — Retrieve scout draft results

### Web Client (`web/`)

Next.js 14 app with:
- Chat interface with SSE streaming
- Network status sidebar (daemon, topology, peers)
- Oracle heartbeat (PING/PONG) testing
- Service worker for background scout coordination
- WebLLM integration for draft token generation

---

## Control Plane Protocol

Defined in `desktop/control_plane/shard_control.proto`:

```protobuf
service ShardControlPlane {
  rpc BroadcastWork(WorkRequest) returns (Ack);
  rpc SubmitResult(WorkResponse) returns (Ack);
  rpc UpdateTopology(TopologyUpdate) returns (Ack);
  rpc Health(google.protobuf.Empty) returns (HealthStatus);
}
```

Currently implemented as HTTP/JSON for simplicity. gRPC can be added later.

---

## Design Constraints

- **No central inference server** — all inference is peer-to-peer
- **Double-dip prevention** — browser detects local .exe and routes to it
- **Verify, Don't Trust** — Oracles verify all Scout drafts
- **Heavier is Truth** — GPU nodes always override browser drafts

---

## Operations Documentation

- Deployment guide: [`docs/deployment-guide.md`](docs/deployment-guide.md)
- Troubleshooting: [`docs/troubleshooting.md`](docs/troubleshooting.md)
- Network topology and scaling: [`docs/network-topology-and-scaling.md`](docs/network-topology-and-scaling.md)

## Production Readiness Roadmap

See [`docs/production-readiness-plan.md`](docs/production-readiness-plan.md) for the phased production plan and review deliverables.

## Next Steps

1. **Load real BitNet model** — Set `BITNET_LIB` and `BITNET_MODEL` env vars
2. **WebLLM integration** — Load a draft model in the browser via WebGPU
3. **Replace file IPC with gRPC** — Use the proto definition for Python↔Rust
4. **Add reputation scoring** — Track Scout accuracy for trust management
5. **Golden Ticket testing** — Inject known-answer prompts for Sybil detection
