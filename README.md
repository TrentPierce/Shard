<div align="center">
  <img src="assets/logo.png" alt="Shard" width="200" />
  <h1>Shard</h1>
  <h3>Browser-Powered Distributed Inference</h3>
  <p>
    Free, unlimited LLM access through a decentralized P2P mesh.<br/>
    Contribute compute from your browser. Earn priority access.
  </p>

  <br/>

  [![CI](https://github.com/TrentPierce/Shard/actions/workflows/ci.yml/badge.svg)](https://github.com/TrentPierce/Shard/actions/workflows/ci.yml)
  [![License: BUSL-1.1](https://img.shields.io/badge/license-BUSL--1.1-blue.svg)](LICENSE)
  [![Version](https://img.shields.io/badge/version-0.4.7-00d4ff.svg)](#)
  [![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)
  [![GitHub Stars](https://img.shields.io/github/stars/TrentPierce/Shard?style=social)](https://github.com/TrentPierce/Shard)

  <br/>

  [**Live Demo**](https://shard-trents-projects-20e9a51a.vercel.app) · [**White Paper**](docs/Shard-White-Paper-Feb-2026.pdf) · [**API Docs**](docs/API.md) · [**Get Started**](#quick-start)
</div>

---

## What Is Shard?

Shard is a **hybrid distributed inference network** that turns every browser tab into an AI compute node. Instead of paying per token to a centralized provider, Shard creates a peer-to-peer mesh where:

- **Scouts** (browser nodes) run lightweight draft models via WebGPU
- **Shards** (desktop/server nodes) run full BitNet 1.58-bit models to verify drafts
- **Leeches** (passive users) consume AI without contributing compute

The result: **server-grade AI quality at zero cost**, powered by the collective compute of anyone who opens the webpage.

---

## Why Shard?

| | Traditional Cloud AI | Shard Network |
|---|---|---|
| **💰 Cost** | $0.002–$0.06 / 1K tokens | Free (compute-for-access) |
| **🔒 Privacy** | Your data on someone else's server | Localhost-first routing |
| **📈 Scalability** | Buy more GPUs | More users = more GPUs |
| **🛡️ Resilience** | Single point of failure | Self-healing P2P mesh |
| **⚡ Latency** | Network RTT + queue wait | Local draft + network verification |
| **🔌 API** | Proprietary | OpenAI-compatible drop-in |

---

## How It Works

```mermaid
flowchart LR
    User[Browser User] --> Web[Next.js Web App]
    Web --> ChatAPI[/API: /v1/chat/completions/]

    ChatAPI --> WorkQ[/API: /v1/scout/work/]
    Web --> WorkQ
    Web --> DraftSubmit[/API: /v1/scout/draft/]
    DraftSubmit --> Verify[Shard Verifier Node\nBitNet Runtime]
    Verify --> ChatAPI

    ChatAPI <--> Sidecar[Rust Sidecar Control Plane]
    Sidecar <--> Mesh[(libp2p Mesh)]
    Mesh <--> Other[Other Shard Nodes]
```

1. **User sends a prompt** → routed to the Shard API
2. **Scouts generate draft tokens** → lightweight models run in-browser via WebGPU
3. **Shards verify drafts** → full BitNet model checks token quality in one parallel pass
4. **Verified tokens stream back** → statistically indistinguishable from a 70B-parameter model

> 📄 For the full technical deep-dive, read the [**White Paper**](docs/Shard-White-Paper-Feb-2026.pdf)

---

## Real-World Use Cases

- 🌐 **Community AI endpoint** — free chatbot powered by browser-contributed compute
- 🎓 **Classroom / lab** — student browsers act as Scouts, one workstation verifies
- 🚀 **Hackathon demo** — one EC2 verifier + attendee browser Scouts
- 🏢 **Internal overflow** — burst capacity when centralized GPU budget is exhausted

---

## New To Shard?

If you want a quick explanation of roles, contribution, and why distributed mode matters, start here:

- [`docs/join-network.md`](docs/join-network.md)
- [`docs/deployment-guide.md`](docs/deployment-guide.md)
- [`/network` leaderboard page](https://shard-trents-projects-20e9a51a.vercel.app/network)

---
## Quick Start

### Fastest Setup Paths

#### Join as Scout (browser contributor)
1. Open the deployed web app.
2. Allow WebGPU when prompted.
3. Keep the tab open; it contributes draft work automatically.

#### Run as Shard (one-command local stack)
```bash
docker compose up --build shard-daemon shard-inference shard-api
```

Verify:
```bash
curl http://localhost:8000/health
curl http://localhost:8000/v1/system/topology
```

### Prerequisites

- **Rust** 1.75+ — [rustup.rs](https://rustup.rs)
- **Python** 3.11+ — with pip
- **Node.js** 18+ — with npm

### Web Client (Browser Scout)

```bash
cd web
npm install
npm run dev
# → http://localhost:3000
```

Your browser enters **Scout mode** automatically — loading a draft model via WebGPU and contributing compute to the mesh.

<details>
<summary><strong>🖥️ Desktop Shard Node (Rust Daemon + Python API)</strong></summary>

#### 1. Start the P2P Daemon

```bash
cd desktop/rust
cargo build --release
./target/release/shard-daemon \
  --control-port 9091 \
  --tcp-port 4001 \
  --webrtc-port 9090 \
  --quic-port 9092
```

#### 2. Start the Inference API

```bash
cd desktop/python
python -m venv .venv
# Windows: .venv\Scripts\activate
# Linux/macOS: source .venv/bin/activate
pip install -r requirements.txt
BITNET_LIB=/path/to/libshard_engine.so \
BITNET_MODEL=/path/to/model.gguf \
python run.py --rust-url http://127.0.0.1:9091
```

</details>

<details>
<summary><strong>🐳 Docker Compose (All-in-One)</strong></summary>

```bash
# Start core services
docker-compose up --build

# With monitoring (Prometheus + Grafana)
docker-compose --profile monitoring up --build
```

Services:
| Service | Port | Description |
|---------|------|-------------|
| `shard-daemon` | 9091, 4001 | P2P networking (libp2p) |
| `shard-api` | 8000 | OpenAI-compatible API |
| `shard-inference` | 7000 | BitNet inference engine |
| `prometheus` | 9095 | Metrics (monitoring profile) |
| `grafana` | 3001 | Dashboards (monitoring profile) |

</details>

<details>
<summary><strong>☁️ Join the Public Network</strong></summary>

Bootstrap peer:

```
/ip4/54.224.107.75/tcp/4001/p2p/12D3KooWPTDTQBH5JTCxhiaZuL9sr695UAEndMDRj9SJ9pi3agEq
```

```bash
./target/release/shard-daemon \
  --control-port 9091 \
  --tcp-port 4001 \
  --webrtc-port 9090 \
  --quic-port 9092 \
  --bootstrap /ip4/54.224.107.75/tcp/4001/p2p/12D3KooWPTDTQBH5JTCxhiaZuL9sr695UAEndMDRj9SJ9pi3agEq
```

</details>

---

## API Usage

Shard is **OpenAI-compatible** — use any existing client library:

```python
from openai import OpenAI

client = OpenAI(
    base_url="http://localhost:8000/v1",
    api_key="your-api-key",  # optional
)

response = client.chat.completions.create(
    model="shard-hybrid",
    messages=[{"role": "user", "content": "Explain quantum computing simply."}],
    stream=True,
)

for chunk in response:
    print(chunk.choices[0].delta.content or "", end="", flush=True)
```

```bash
# Or with cURL
curl http://localhost:8000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model": "shard-hybrid", "messages": [{"role": "user", "content": "Hello!"}]}'
```

> 📖 Full API reference: [**docs/API.md**](docs/API.md)

---

## Architecture

| Component | Language | Purpose |
|-----------|----------|---------|
| **Web Client** | TypeScript / React | Browser UI, WebLLM scout engine, P2P mesh |
| **Shard API** | Python (FastAPI) | OpenAI-compatible API, inference orchestration |
| **Rust Daemon** | Rust (libp2p) | P2P networking, peer discovery, gossipsub |
| **BitNet Bridge** | Python (ctypes) | Local model verification via BitNet 1.58-bit |

### Key Technologies

- **[BitNet b1.58](https://arxiv.org/abs/2402.17764)** — 1.58-bit ternary quantization (6–8x VRAM reduction)
- **[libp2p](https://libp2p.io/)** — Transport-agnostic P2P networking (TCP, WebSocket, WebRTC, QUIC)
- **[WebLLM](https://webllm.mlc.ai/)** — In-browser LLM inference via WebGPU
- **Hybrid Speculative Decoding** — Scout drafts + Shard verification in one parallel pass
- **Golden Ticket Protocol** — Sybil attack prevention via random audit injections

> 📐 Detailed architecture: [**docs/ARCHITECTURE.md**](docs/ARCHITECTURE.md)

---

## Response Quality

For non-Llama-3 models (e.g., TinyLlama GGUF):
```bash
SHARD_PROMPT_FORMAT=chatml
```

For Llama-3 chat models:
```bash
SHARD_PROMPT_FORMAT=llama3  # or 'auto' with matching model names
```

Fallback for generic instruct models:
```bash
SHARD_PROMPT_FORMAT=plain
```

---

## Python SDK

> ⚠️ **Experimental** — The Python SDK is currently in scaffolding stage.

```bash
cd python-sdk
pip install -e .
```

See [`python-sdk/README.md`](python-sdk/README.md) for transport assumptions and limitations.

---

## Contributing

We welcome contributions! See [**CONTRIBUTING.md**](CONTRIBUTING.md) for:

- Development workflow and branching strategy
- Style guides (Python, Rust, TypeScript)
- Testing requirements
- Commit message format

```bash
# Quick start for contributors
make setup    # Install all dependencies
make dev      # Start all services
make test     # Run all test suites
```

### Versioning

Shard uses a single source of truth in the root `VERSION` file.

```bash
make version         # show current version
make version-sync    # sync all package/app versions from VERSION
make version-set V=0.4.6
```

---

## Community

- 📋 [**Issues**](https://github.com/TrentPierce/Shard/issues) — Bug reports and feature requests
- 💬 [**Discussions**](https://github.com/TrentPierce/Shard/discussions) — Questions, ideas, general chat
- 📖 [**Docs**](docs/) — Architecture, deployment, troubleshooting
- 📜 [**Changelog**](CHANGELOG.md) — Version history

Issue templates are in [`.github/ISSUE_TEMPLATE/`](.github/ISSUE_TEMPLATE/).

---

## License

[Business Source License 1.1](LICENSE) — Free for non-competing use under 10K requests/month.  
Converts to Apache 2.0 on February 13, 2036.

---

<div align="center">
  <sub>Built with 🧊 by the <a href="https://github.com/TrentPierce/Shard">Shard</a> community</sub>
</div>

