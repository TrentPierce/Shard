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

## Release-Candidate Snapshot (March 6, 2026)

Latest refreshed RC matrix:

- 3 runs per scenario
- 16 browser scouts configured
- 2 req/s
- 10s runs
- local + EC2 verifier pools
- strict readiness and queue flush between runs

| Scenario | p95 latency (ms) | Throughput (TPS) | Error rate (%) | Speculative samples |
|---------|-------------------|------------------|----------------|---------------------|
| 1 node, no scouts | 284.929 | 2.1000 | 0.0000 | 0 |
| 1 node, with scouts | 1089.169 | 2.1000 | 0.0000 | 3 |
| 2 nodes, no scouts | 417.335 | 2.1000 | 0.0000 | 0 |
| 2 nodes, with scouts | 1134.846 | 2.1000 | 0.0000 | 0 |

Current recommendation: **NO_GO**

What this means:

- The verifier mesh is stable in all four scenarios.
- The benchmark harness now isolates verifier-only runs by forcing `standard` mode and disabling scout ingress during no-scout scenarios.
- Scouts no longer break the network, but they still do not beat the verifier-only baseline in this RC profile.
- At this low 2 req/s load, the fastest p95 is still `1 node, no scouts`.

Raw artifacts:

- `reports/release-rc/release-rc-20260306T053407Z-20260306Tclean-zero-scouts-isolated/go-no-go-summary.json`
- `reports/release-rc/release-rc-20260306T053407Z-20260306Tclean-zero-scouts-isolated/go-no-go-report.md`
- `reports/release-rc/release-rc-20260306T053407Z-20260306Tclean-zero-scouts-isolated/one-node-no-scouts-run1.json`
- `reports/release-rc/release-rc-20260306T053407Z-20260306Tclean-zero-scouts-isolated/one-node-with-scouts-run1.json`
- `reports/release-rc/release-rc-20260306T053407Z-20260306Tclean-zero-scouts-isolated/two-node-no-scouts-run1.json`
- `reports/release-rc/release-rc-20260306T053407Z-20260306Tclean-zero-scouts-isolated/two-node-with-scouts-run1.json`

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


