<div align="center">
  <img src="docs/assets/logo.png" alt="Shard Network" width="160" />
  <h1>Shard Network</h1>
  <p><strong>Local-first AI routing with browser answers, desktop inference, and experimental WAN scouts.</strong></p>

  [![CI/CD](https://github.com/TrentPierce/Shard/actions/workflows/ci.yml/badge.svg)](https://github.com/TrentPierce/Shard/actions/workflows/ci.yml)
  [![Version](https://img.shields.io/badge/version-0.6.5-blue.svg)](https://github.com/TrentPierce/Shard/releases/tag/v0.6.5)
  [![License: BSL 1.1](https://img.shields.io/badge/License-BSL%201.1-blue.svg)](LICENSE)

  [Live Network](https://shardnetwork.live) &nbsp;·&nbsp; [Quick Start](#quick-start) &nbsp;·&nbsp; [Docs](docs/) &nbsp;·&nbsp; [Python SDK](#python-sdk)
</div>

---

## What Is Shard?

Shard is an OpenAI-compatible AI runtime that starts in the browser and escalates to desktop verifier nodes only when the work is too complex, too long, or too stateful for the local path.

- Browser local-first router: lightweight prompts can complete entirely in the browser with no network round-trip.
- Desktop verifier nodes: Rust daemons run heavier inference and the default network execution path.
- Experimental WAN scouts: browser draft and verify experiments remain available behind explicit benchmark flows, but they are no longer the main product path.

Clients still use the standard `/v1/chat/completions` API.

```text
Browser router
  -> local answer in browser
  -> or compacted network request
  -> desktop verifier daemon
  -> final response

Experimental WAN scout path remains opt-in:
  benchmark scout browser -> draft tokens -> verifier -> validated response
```

---

## Quick Start

### Browser App

1. Open [shardnetwork.live/chat](https://shardnetwork.live/chat).
2. Leave the mode selector on `Auto` for the normal product path.
3. Use `Browser Only` to force a local browser response.
4. Use `Network Only` to force verifier-only routing.
5. Use `Experimental WAN` only when you have explicitly prepared the benchmark scout path.

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

### Experimental WAN Scout

Use [docs/REMOTE_LLAMA_SCOUT_TEST_RUNBOOK.md](docs/REMOTE_LLAMA_SCOUT_TEST_RUNBOOK.md) only when you want to benchmark the experimental browser-scout path. It is not required for normal chat or node operation.

---

## Current Benchmark Position (March 11, 2026)

These are the defensible benchmark statements today:

| Scenario | Result | What it means |
| --- | --- | --- |
| Browser local-first chat | Product default | Simple prompts can complete entirely in-browser with no network round-trip. |
| Local Llama 8B verifier baseline (`10 vs 10`, same machine) | `11295.1 ms` average, `11297 ms` median | Current verifier-only reference for the experimental WAN comparison. |
| Experimental WAN Llama scout (`10 vs 10`, live site, same machine) | `12004.4 ms` average, `11888 ms` median | Correctness is strong, but wall-clock is still slower than baseline. |
| Experimental WAN verification quality (`10 vs 10`, same machine) | `10/10` wait hits, `4/4` accepted draft tokens on every run | The compatible Llama pair is real and repeatable. |
| Browser scout timing after reuse patch | First request: `prefill_ms=258`, `decode_ms=119`, `submit_ms=9`; repeated identical requests: `generate_ms=0`, `reuse=exact_prompt_cache` | Prompt-state reuse works and removes repeated identical browser generation cost after the first hit. |
| Browser Qwen draft against local Qwen 9B verifier | Rejected in strict mode | This pair is not a safe speculative match. |

### What we can claim today

- Local-first browser routing is the product architecture.
- Harder prompts can escalate cleanly to a desktop verifier path.
- The compatible Llama experimental WAN scout path is correct and repeatable.
- The same-machine experimental WAN path is still slower overall than verifier-only baseline, even with perfect `4/4` acceptance on every measured run.
- Exact prompt caching and prompt-state reuse materially reduce repeated identical browser-scout cost.

### What we are not claiming yet

- WAN browser scouts as the default fast path.
- Universal speedups from browser scouts over a network.
- Production uplift for unverified draft and verifier model pairs.

---

## Key Features

| Feature | Description |
| --- | --- |
| OpenAI-compatible API | Drop-in `/v1/chat/completions` interface |
| Local-first router | Browser decides between a local answer and a network escalation |
| Browser-owned context compaction | Older chat turns are summarized and trimmed before network escalation |
| Desktop heavy inference | Rust verifier daemon handles larger prompts and longer generations |
| Experimental WAN scouts | Opt-in browser draft path for compatibility and benchmark work |
| libp2p mesh networking | Multi-seed bootstrap, discovery, health sharing, and request forwarding for non-speculative routes |
| Observability | Metrics, structured logs, speculative traces, and benchmark harnesses |
| Python SDK | Typed client for OpenAI-compatible integrations |

---

## Architecture

```text
User prompt
  -> browser router
     -> local browser answer
     -> or compacted request to verifier daemon
        -> standard or local speculative execution
        -> final response

Experimental WAN:
  benchmark scout browser
    -> draft tokens
    -> verifier validation
    -> final response
```

### Request flow

1. A user submits a prompt in the browser.
2. The browser scores prompt complexity and decides `local_answer`, `network_route`, or `network_route_with_compaction`.
3. If the prompt stays local, WebLLM answers directly in the browser.
4. If the prompt escalates, the browser sends raw or compacted messages to a verifier daemon.
5. The daemon resolves `standard`, `local_speculative`, or `experimental_wan` execution.
6. The final response is returned through the same OpenAI-compatible API.

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

Business Source License 1.1 (BSL 1.1). See [LICENSE](LICENSE).
