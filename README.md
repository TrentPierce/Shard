<div align="center">
  <img src="docs/assets/logo.png" alt="Shard Network" width="160" />
  <h1>Shard Network</h1>
  <p><strong>Distributed AI inference where browser scouts draft and verifier nodes validate.</strong></p>

  [![CI/CD](https://github.com/TrentPierce/Shard/actions/workflows/ci.yml/badge.svg)](https://github.com/TrentPierce/Shard/actions/workflows/ci.yml)
  [![Version](https://img.shields.io/badge/version-0.6.5-blue.svg)](https://github.com/TrentPierce/Shard/releases/tag/v0.6.5)
  [![License: BSL 1.1](https://img.shields.io/badge/License-BSL%201.1-blue.svg)](LICENSE)

  [Live Network](https://shardnetwork.live) &nbsp;·&nbsp; [Quick Start](#quick-start) &nbsp;·&nbsp; [Docs](docs/) &nbsp;·&nbsp; [Python SDK](#python-sdk)
</div>

---

## What Is Shard?

Shard is an OpenAI-compatible distributed inference network with two participant types:

- Browser scouts: WebGPU-capable browsers that generate speculative draft tokens.
- Verifier nodes: Rust daemons that validate drafts, finish generation, and serve the final response.

Clients use the standard `/v1/chat/completions` API.

```text
Browser scouts -> speculative drafts -> verifier nodes -> validated responses -> API clients
```

---

## Quick Start

### Browser Scout

1. Open [shardnetwork.live](https://shardnetwork.live).
2. Click **Join**.
3. Let the browser model finish downloading.
4. Keep the tab open while the page shows **contributing** or **ready**.

### Desktop Verifier

1. Download the latest **Shard GUI** from [GitHub Releases](https://github.com/TrentPierce/Shard/releases/latest).
2. Let the app download the verifier model on first run.
3. Save settings, restart once, then click **Start**.
4. Confirm `http://127.0.0.1:9091/health` returns `status: ok`.

### Docker Verifier

```bash
git clone https://github.com/TrentPierce/Shard.git
cd Shard
docker compose up --build shard-daemon -d
curl http://localhost:9091/health
```

Required open ports: `4001/tcp`, `9091/tcp`, `9090/udp`, `9092/udp`

Full node setup: [docs/run-a-node.md](docs/run-a-node.md)

---

## Current Benchmark Position (March 10, 2026)

These are the most defensible benchmark statements today:

| Scenario | p95 latency | Throughput | Error rate | What it means |
| --- | ---: | ---: | ---: | --- |
| 3 Fly verifier nodes, verifier-only | 986.605 ms | 0.55 TPS | 0.00% | Current fast-node baseline |
| 3 Fly verifier nodes, browser scouts attached | 965.127 ms | 0.55 TPS | 0.00% | Fast nodes stay neutral because browser scouts back off when fast-verifier bypass is active |
| Local Llama 8B verifier with remote browser scout (`10 vs 10`) | 9936.093 ms median | 9890.574 ms average | 0.00% | Compatible Llama browser scouts delivered accepted speculative tokens on all 10 runs and produced a small but measurable latency win over local-only baseline |
| Browser Qwen draft against local Qwen 9B verifier | Rejected in strict mode | N/A | N/A | This pair is not currently a safe speculative match |

### What we can claim today

- Fast verifier nodes should keep adaptive browser-scout bypass enabled.
- Browser scouts are proven neutral on fast Fly-class verifier nodes when bypass is active.
- The Llama browser-draft path now works end to end against a larger local verifier and has a repeated remote benchmark showing a small but measurable latency improvement on a compatible pair.
- Qwen browser-draft pairing should remain strict or disabled until a verified compatible browser draft model exists.

### What we are not claiming yet

- Universal browser-scout speedups on fast nodes.
- Universal browser-scout speedups on all slower nodes.
- Production uplift for unverified draft/verifier model pairs.

### Representative artifacts

- `benchmarks/fly-three-node-standard-v3.json`
- `benchmarks/fly-three-node-browser-scouts-v9.json`
- `baseline_qwen9b_no_scout.json`
- `baseline_qwen9b_live_scout.json`

---

## Key Features

| Feature | Description |
| --- | --- |
| OpenAI-compatible API | Drop-in `/v1/chat/completions` interface |
| Browser-powered drafting | WebGPU scouts can draft speculative tokens when the verifier pair is compatible |
| Verification gatekeeping | Drafts are scored and either accepted or rejected before response tokens are finalized |
| libp2p mesh networking | Multi-seed bootstrap, gossip-based discovery, and mesh forwarding |
| Adaptive scout bypass | Fast verifiers stay neutral by refusing speculative waits that are not profitable |
| Observability | Metrics, structured logs, speculative traces, and benchmark harnesses |
| Python SDK | Typed client for OpenAI-compatible integrations |

---

## Architecture

```text
Browser scouts
  -> draft tokens
  -> verifier daemon (Rust)
  -> validated tokens / final response
  -> API client

Verifier nodes also participate in a libp2p mesh for bootstrap, health sharing, and request forwarding.
```

### Request flow

1. A client sends a prompt to a verifier node.
2. If a compatible browser scout is available and the verifier decides the wait is profitable, the verifier issues speculative work.
3. The verifier validates any returned draft tokens and continues generation.
4. The final response is returned through the same OpenAI-compatible API.

---

## Development

```bash
make setup
make dev
make test
make lint
make docker
```

Useful targets:

```bash
make dev-daemon
make dev-web
make test-rust
make test-web
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

```text
desktop/rust/       Verifier daemon, mesh, scheduler, verifier crates
web/                Next.js website, scout UI, OpenAI-compatible proxy routes
sdk/python/         Typed Python client
cpp/                llama.cpp bridge and native inference helpers
benchmarks/         Benchmark harnesses and scenario runners
deploy/             Docker, Fly, release, monitoring, and infra assets
installers/         Desktop packaging and installer assets
scripts/            Build, release, deploy, and developer helpers
docs/               Architecture, runbooks, and operational guidance
```

---

## Documentation

| Guide | Description |
| --- | --- |
| [docs/run-a-node.md](docs/run-a-node.md) | Node operator quickstart |
| [docs/mesh-benchmark.md](docs/mesh-benchmark.md) | Local Docker mesh scaling guide |
| [docs/REMOTE_LLAMA_SCOUT_TEST_RUNBOOK.md](docs/REMOTE_LLAMA_SCOUT_TEST_RUNBOOK.md) | Remote Llama browser-scout test procedure |
| [docs/release-rc-checklist.md](docs/release-rc-checklist.md) | Release checklist |
| [docs/release-rc-runbook.md](docs/release-rc-runbook.md) | RC execution and rollback guide |
| [docs/architecture.md](docs/architecture.md) | System design and request flow |
| [docs/deployment.md](docs/deployment.md) | Environment variables and deployment setup |
| [docs/api.md](docs/api.md) | API reference |
| [docs/verification-protocol.md](docs/verification-protocol.md) | Draft validation protocol |
| [docs/contributing.md](docs/contributing.md) | Contribution guide |

---

## License

Business Source License 1.1 (BSL 1.1). See [LICENSE](LICENSE).

