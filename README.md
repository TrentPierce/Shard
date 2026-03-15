<div align="center">
  <img src="docs/assets/logo.png" alt="Shard Network" width="160" />
  <h1>Shard Network</h1>
  <p><strong>Receipt-first workflow observability for AI agents running across personal, private, and public capacity.</strong></p>

  [![CI/CD](https://github.com/TrentPierce/Shard/actions/workflows/ci.yml/badge.svg)](https://github.com/TrentPierce/Shard/actions/workflows/ci.yml)
  [![Version](https://img.shields.io/badge/version-0.6.6-blue.svg)](https://github.com/TrentPierce/Shard/releases/tag/v0.6.6)
  [![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](LICENSE)

  [Live Network](https://shardnetwork.live) &nbsp;·&nbsp; [Quick Start](#quick-start) &nbsp;·&nbsp; [Docs](docs/) &nbsp;·&nbsp; [Python SDK](#python-sdk)
</div>

---

## What Is Shard?

Shard is an agent execution runtime that helps software decide where each step of a workflow should run:

- `personal`: your own laptop or workstation
- `private`: your team or company-owned Shard nodes
- `public`: shared specialist capacity on the broader Shard mesh

The first Shard V1 workflow is `research_brief`.

You submit a question, a bundle of source documents, and a routing policy. Shard returns:

- the final brief
- an append-only receipt chain
- a provenance graph that explains where each step ran
- the fallback path if anything went wrong
- latency, cost, trust tier, and selected candidate metadata for each step

The goal is simple: make multi-step agent workflows understandable instead of opaque.

## Why It Matters

Most AI platforms can tell you the answer. Very few can tell you, in plain terms:

- why a task used your own machine instead of the public market
- why a public specialist was chosen for synthesis
- what fallback fired when a node failed
- how much the degraded path cost

Shard treats those answers as product features, not hidden scheduler trivia.

## What Makes Shard Different

| Capability | What it means |
| --- | --- |
| Receipt-first execution | Every workflow step emits a durable receipt with routing, trust, cost, latency, and failure details. |
| Reconstructable provenance | The graph is rebuilt from `parent_receipt_id` links rather than coordinator-only state. |
| Cross-topology routing | One workflow can use personal, private, and public capacity under explicit policy. |
| Graceful degradation | Failed and orphaned paths stay visible instead of disappearing behind a generic error. |
| Familiar compatibility layer | `/v1/chat/completions` still works while the workflow APIs provide the differentiated surface. |

## Quick Start

### 1. Run the provenance demo

Open [shardnetwork.live/provenance](https://shardnetwork.live/provenance).

This is the clearest way to understand Shard V1:

1. Enter a research question.
2. Paste a few source documents.
3. Choose your supply tiers, trust floor, and budget guardrails.
4. Run the workflow and inspect the returned brief, receipts, and provenance graph.

### 2. Add your own capacity

1. Download the latest **Shard GUI** from [GitHub Releases](https://github.com/TrentPierce/Shard/releases/latest).
2. Let the local model finish downloading on first run.
3. Save settings, restart once, then click **Start**.
4. Confirm `http://127.0.0.1:9091/health` returns `status: ok`.

That node can then serve personal, private, or public work depending on policy and deployment mode.

### 3. Integrate the API

Use the compatibility surface when you just need chat:

- `POST /v1/chat/completions`

Use the workflow surface when you need routing evidence:

- `POST /v1/agents/tasks`
- `GET /v1/executions/{execution_id}`
- `GET /v1/executions/{execution_id}/receipts`
- `GET /v1/executions/{execution_id}/provenance`
- `GET /v1/capabilities`

## The V1 Workflow

`research_brief` is intentionally opinionated.

It does three things:

1. Plans the work by choosing sub-questions and the most relevant source IDs.
2. Prefers cheaper personal or private nodes for source summarization when policy allows.
3. Uses a stronger specialist candidate for synthesis when the trust and budget policy allow it.

The final artifact includes:

- `brief`
- `planner_notes`
- `sub_questions`
- `selected_source_ids`
- `source_summaries`

## Product Status

Shard V1 is centered on workflow observability, not agent economics.

That means:

- receipts and provenance are in scope
- graceful degradation is in scope
- policy-aware routing across personal, private, and public supply is in scope
- wallet-native settlement and agent-to-agent economics are deferred to a later release

## Legacy Paths

Shard still contains browser-local chat, mesh forwarding, and experimental scout research work.

Those capabilities remain useful, but they are no longer the main product story. The main story is:

**policy-aware agent workflows with receipt-carrying execution**

---

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
pip install shardnetwork-client
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

Programmatic contribution is also available through the SDK:

```python
from shard import ShardClient

client = ShardClient(base_url="http://localhost:9091")
contributor = client.contribution.create_session()
contributor.set_participation(True)
contributor.register_node(role="verifier", capacity=1)
contributor.heartbeat(
    role="verifier",
    queue_depth=0,
    node_latency_ms=24,
    uptime_seconds=15,
    capability_tier="gpu_fast",
    gpu_available=True,
    public_api=True,
)
```

That lets developers integrate both sides of the network:

- consume inference with `/v1/chat/completions`
- contribute verifier capacity with the signed contributor control plane

---

## Repo Structure

```text
desktop/rust/       Verifier daemon, scheduler, mesh, and desktop app crates
web/                Next.js app, browser router, local chat runtime, and benchmark scout UI
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
| [docs/architecture.md](docs/architecture.md) | Local-first request flow and system boundaries |
| [docs/run-a-node.md](docs/run-a-node.md) | Verifier node quickstart |
| [docs/api.md](docs/api.md) | API contracts and inference-mode headers |
| [docs/verification-protocol.md](docs/verification-protocol.md) | How speculative draft tokens are validated when speculative mode is enabled |
| [docs/NETWORK_PERFORMANCE_ROADMAP.md](docs/NETWORK_PERFORMANCE_ROADMAP.md) | Performance roadmap after the local-first pivot |
| [docs/REMOTE_LLAMA_SCOUT_TEST_RUNBOOK.md](docs/REMOTE_LLAMA_SCOUT_TEST_RUNBOOK.md) | Experimental WAN Llama scout procedure |
| [docs/REMOTE_LLAMA_SCOUT_RESULT_2026-03-11.md](docs/REMOTE_LLAMA_SCOUT_RESULT_2026-03-11.md) | March 11, 2026 experimental WAN benchmark notes |
| [docs/deployment.md](docs/deployment.md) | Environment variables and deployment setup |
| [docs/contributing.md](docs/contributing.md) | Contribution guide |

---

## License

Functional Source License 1.1 (FSL-1.1-ALv2). See [LICENSE](LICENSE) and [LICENSING.md](LICENSING.md).
