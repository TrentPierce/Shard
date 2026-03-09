<div align="center">
  <img src="docs/assets/logo.png" alt="Shard Network" width="160" />
  <h1>Shard Network</h1>
  <p><strong>Distributed AI inference — browser Scouts generate speculative drafts, Verifier nodes validate and stream responses.</strong></p>

  [![CI/CD](https://github.com/TrentPierce/Shard/actions/workflows/ci.yml/badge.svg)](https://github.com/TrentPierce/Shard/actions/workflows/ci.yml)
  [![Version](https://img.shields.io/badge/version-0.6.2-blue.svg)](https://github.com/TrentPierce/Shard/releases/tag/v0.6.2)
  [![License: BSL 1.1](https://img.shields.io/badge/License-BSL%201.1-blue.svg)](LICENSE)

  [Live Network](https://shardnetwork.live) &nbsp;·&nbsp; [Quick Start](#quick-start) &nbsp;·&nbsp; [Docs](docs/) &nbsp;·&nbsp; [Python SDK](#python-sdk)
</div>

---

## What is Shard?

Shard is an **OpenAI-compatible distributed inference network** that combines two types of participants:

- **Browser Scouts** — any WebGPU-capable browser that generates fast speculative draft tokens at no server cost
- **Verifier Nodes** — server daemons that validate drafts via KL-divergence scoring, complete generation, and stream final responses

Clients interact through a standard `/v1/chat/completions` API — no changes to your existing code.

```
Browser Scouts  →  speculative drafts  →  Verifier Nodes  →  validated stream  →  API clients
```

---

## Quick Start

### Browser Scout � easiest path

1. Open [shardnetwork.live](https://shardnetwork.live)
2. Click **Join**
3. Wait for the browser model download to finish
4. Keep the tab open while the page shows **contributing**

### Desktop Verifier � easiest full node path

1. Download the latest **Shard GUI** from [GitHub Releases](https://github.com/TrentPierce/Shard/releases/latest)
2. Let the app download the verifier model on first run
3. Save settings, restart the node once, then click **Start**
4. Confirm `http://127.0.0.1:9091/health` returns `status: ok`

### Verifier Node � Docker

```bash
git clone https://github.com/TrentPierce/Shard.git
cd Shard
docker compose up --build shard-daemon -d
curl http://localhost:9091/health
```

Required open ports: `4001/tcp`, `9091/tcp`, `9090/udp`, `9092/udp`

See [docs/run-a-node.md](docs/run-a-node.md) for the full quickstart, including binary install and health checks.

### Local Mesh Scale Test (Docker)

Use the local mesh profile to compare `1` vs `2` vs `N` verifier nodes:

```powershell
.\deploy\demo\mesh-up.ps1 -Nodes 1
.\deploy\demo\mesh-up.ps1 -Nodes 2
.\deploy\demo\mesh-up.ps1 -Nodes 5
```

Full guide: [docs/mesh-benchmark.md](docs/mesh-benchmark.md)

---

## Latest Benchmark Snapshot (March 9, 2026)

Latest validated benchmark results:

- pinned 3-node Fly verifier comparison on `performance-4x` machines
- pinned browser scouts attached to the same 3 Fly nodes
- one-node long-generation local comparison for slower hardware
- all numbers below are from the latest validated harness runs, not from the current live `shardnetwork.live` benchmark page

| Scenario | p95 latency (ms) | Throughput (TPS) | Error rate (%) | Interpretation |
|---------|-------------------|------------------|----------------|----------------|
| 3 Fly nodes, verifier-only | 986.605 | 0.5500 | 0.0000 | Current fast-node baseline |
| 3 Fly nodes, browser scouts attached | 965.127 | 0.5500 | 0.0000 | Fast nodes stay neutral because scouts back off when the verifier bypass is active |
| 1 slower local node, verifier-only | 7958.929 | 0.2167 | 0.0000 | Current slow-node long-generation baseline |
| 1 slower local node, browser scouts active | 3471.707 | 0.2167 | 0.0000 | Browser scouts reduce tail latency on slower hardware |

Current recommendation:

- Fast verifier nodes should keep adaptive browser-scout bypass enabled.
- Browser scouts are currently useful on slower nodes, not on fast Fly-class verifier nodes.
- Public uplift claims should stay scoped to slower hardware until Fly-class nodes show a repeatable net gain.

What this means:

- The 3-node Fly mesh is stable and benchmarkable.
- Fast Fly nodes no longer regress when browser scouts are attached because the daemon now bypasses speculative waits and the browser scout loop backs off instead of polling aggressively.
- Slower nodes remain the best current target for browser scouts. The latest long-generation local check cut p95 from `7.96s` to `3.47s`.
- The live `shardnetwork.live/benchmark/scout` page still has a WebLLM asset-route issue (`500` on `/api/webllm/model/.../mlc-chat-config.json`), so the pinned harness remains the source of truth for benchmark publication.

Raw artifacts:

- `benchmarks/fly-three-node-standard-v3.json`
- `benchmarks/fly-three-node-browser-scouts-v9.json`
- `benchmarks/local-long-browser-uplift/one-node-no-scouts-clean.json`
- `benchmarks/local-long-browser-uplift/one-node-browser-scouts.json`

---
## Key Features

| Feature | Description |
|---------|-------------|
| OpenAI-compatible API | Drop-in `/v1/chat/completions` — no client changes needed |
| Browser-powered drafting | WebGPU Scouts reduce verifier compute load via speculative decoding |
| KL-divergence validation | Drafts are statistically scored before tokens are finalized |
| libp2p mesh networking | Peer-to-peer bootstrap ring with canary rollout support |
| Overflow routing | Circuit breaker + SLA enforcement for burst traffic |
| Observability built-in | Prometheus metrics, Grafana dashboards, structured logs |
| Python SDK | Typed httpx + pydantic client, fully OpenAI-compatible |

---

## Architecture

```
┌──────────────────────────────────────────────────────────┐
│                      Shard Network                       │
│                                                          │
│  ┌─────────────┐    draft tokens    ┌─────────────────┐ │
│  │   Browser   │ ────────────────►  │    Verifier     │ │
│  │   Scouts    │                    │  Daemon (Rust)  │ │
│  │  (WebGPU)   │ ◄────────────────  │  libp2p mesh    │ │
│  └─────────────┘   validated ACK    └────────┬────────┘ │
│                                              │          │
│                                    OpenAI-compatible API │
│                                              │          │
│                                    ┌─────────▼────────┐ │
│                                    │   Your Clients   │ │
│                                    └──────────────────┘ │
└──────────────────────────────────────────────────────────┘
```

**Request flow:**
1. Client submits a prompt to a Verifier's `/v1/chat/completions` endpoint
2. Active Scouts draft candidate tokens locally in the browser using WebGPU
3. Verifier scores drafts with KL-divergence, accepts or rejects, and streams validated tokens back

---

## Development

```bash
make setup      # install all dependencies (Rust + web)
make dev        # start daemon + web UI locally
make test       # run all test suites (Rust + web + Python)
make lint       # run all linters
make docker     # start full stack via Docker Compose
```

Individual targets:

```bash
make dev-daemon   # Rust daemon only (port 9091)
make dev-web      # Next.js UI only (port 3000)
make test-rust    # cargo test
make test-web     # jest
```

---

## Python SDK

```bash
pip install -e sdk/python
```

```python
from shard import ShardClient

client = ShardClient(base_url="http://localhost:9091")
response = client.chat.completions.create(
    model="default",
    messages=[{"role": "user", "content": "Hello"}],
)
print(response.choices[0].message.content)
```

---

## Repo Structure

```
desktop/rust/       Verifier daemon — libp2p mesh, API, bootstrap ring, canary rollout
  crates/           Modular crates: common, crypto, gateway, ledger, metrics, network, scheduler, verifier
  daemon/           Binary entrypoint
web/                Next.js Scout UI + OpenAI-compatible proxy API routes
sdk/python/         Typed Python client (OpenAI-compatible, httpx + pydantic)
cpp/                llama.cpp inference library + C bridge
integrations/       Overflow router with circuit breaker and SLA enforcement
benchmarks/         Distributed benchmark harness and scenario runner
deploy/             Docker Compose, Terraform, Kubernetes, Prometheus + Grafana configs
installers/         Platform packages: Linux, macOS, Windows, Homebrew, winget
scripts/            Build, release, version sync, and signing automation
tests/              Root-level Python tests (verification engine, credit system)
docs/               Architecture, deployment, API, and operations documentation
```

---

## Documentation

| Guide | Description |
|-------|-------------|
| [run-a-node.md](docs/run-a-node.md) | Quickstart for new node operators |
| [mesh-benchmark.md](docs/mesh-benchmark.md) | Local Docker mesh scaling and performance comparison |
| [release-rc-checklist.md](docs/release-rc-checklist.md) | Public release go/no-go checklist and RC matrix command |
| [release-rc-runbook.md](docs/release-rc-runbook.md) | Step-by-step RC execution, parity checks, and rollback commands |
| [gui-audit.md](docs/gui-audit.md) | GUI readiness audit and remediation summary |
| [architecture.md](docs/architecture.md) | System design and request flow |
| [deployment.md](docs/deployment.md) | Environment variables and HA setup |
| [api.md](docs/api.md) | API reference |
| [verification-protocol.md](docs/verification-protocol.md) | Draft token validation protocol |
| [contributing.md](docs/contributing.md) | Contribution guidelines |
| [sla.md](docs/sla.md) | SLA definition and thresholds |
| [enterprise-vpc-deployment.md](docs/enterprise-vpc-deployment.md) | Private VPC deployment |

---

## License

Business Source License 1.1 (BSL 1.1). See [LICENSE](LICENSE).

