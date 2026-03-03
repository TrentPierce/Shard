<div align="center">
  <img src="docs/assets/logo.png" alt="Shard" width="180" />
  <h1>Shard Network</h1>
  <p><strong>Distributed AI inference — browser Scouts generate drafts, Verifier nodes validate and stream responses.</strong></p>
</div>

[![CI/CD](https://github.com/TrentPierce/Shard/actions/workflows/ci.yml/badge.svg)](https://github.com/TrentPierce/Shard/actions/workflows/ci.yml)
[![Version](https://img.shields.io/badge/version-0.6.2-blue.svg)](https://github.com/TrentPierce/Shard/releases/tag/v0.6.2)
[![License: BSL 1.1](https://img.shields.io/badge/License-BSL%201.1-blue.svg)](LICENSE)

---

## How It Works

1. **Scout** (browser, WebGPU) — joins the network via `shardnetwork.live`, generates speculative draft tokens.
2. **Verifier** (daemon node) — validates drafts using KL-divergence, completes generation, streams the response.
3. Clients call a standard **OpenAI-compatible API** (`/v1/chat/completions`).

---

## Contribute in Under 60 Seconds

### Scout — no install required

1. Open [shardnetwork.live](https://shardnetwork.live)
2. Click **Join** or **Start Contributing**
3. Keep the tab open to contribute browser GPU compute

### Verifier Node — Docker

```bash
git clone https://github.com/TrentPierce/Shard.git
cd Shard
docker compose up --build shard-daemon -d
curl http://localhost:9091/health
```

Required open ports: `4001/tcp`, `9091/tcp`, `9090/udp`, `9092/udp`

See [docs/run-a-node.md](docs/run-a-node.md) for the full quickstart, including binary install and health verification.

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

## Documentation

| Guide | Description |
|-------|-------------|
| [run-a-node.md](docs/run-a-node.md) | Quickstart for new node operators |
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
