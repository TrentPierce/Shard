# Run a Shard Verifier Node

This guide gets a Shard verifier node running in a few minutes.

The verifier is the heavy-inference worker in the current Shard architecture. Browser sessions may answer easy prompts locally, then escalate harder prompts to your daemon when more compute is needed.

## Prerequisites

- Docker and Docker Compose, or a local binary build
- A publicly reachable host if you want remote traffic
- Ports open: `4001/tcp`, `9091/tcp`, `9090/udp`, `9092/udp`

## Quick Start (Docker)

```bash
git clone https://github.com/TrentPierce/Shard.git
cd Shard
docker compose up --build shard-daemon -d
```

## Quick Start (Binary)

```bash
curl -sSL https://shard.network/install.sh | bash
shard-daemon --contribute
```

## Verify It Is Running

```bash
curl http://localhost:9091/health
```

A healthy response looks like:

```json
{ "status": "ok", "rust_version": "0.6.5" }
```

## Test Inference

Verifier-only baseline:

```bash
curl http://localhost:9091/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "X-Shard-Inference-Mode: standard" \
  -d '{"model": "default", "messages": [{"role": "user", "content": "Hello"}]}'
```

Default network acceleration path:

```bash
curl http://localhost:9091/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "X-Shard-Inference-Mode: local_speculative" \
  -d '{"model": "default", "messages": [{"role": "user", "content": "Explain Shard in one paragraph."}]}'
```

Experimental WAN scout path:

```bash
curl http://localhost:9091/v1/chat/completions \
  -H "Content-Type: application/json" \
  -H "X-Shard-Inference-Mode: experimental_wan" \
  -d '{"model": "default", "messages": [{"role": "user", "content": "Explain Shard in one paragraph."}]}'
```

Use `experimental_wan` only when a benchmark scout is explicitly attached. It is not required for normal node operation.

## Model Provisioning Note

On first launch, ShardGUI auto-downloads the default verifier model from `deploy/models/manifest.json` and verifies its SHA-256 hash before enabling contribution. If the manifest endpoint is unavailable, ShardGUI falls back to a built-in default model URL. After download completes, ShardGUI restarts the node automatically and retries startup if the first restart does not come online.

## Required Ports

| Port | Protocol | Purpose |
|------|----------|---------|
| 4001 | TCP | libp2p mesh peering |
| 9091 | TCP | HTTP API and health |
| 9090 | UDP | QUIC transport |
| 9092 | UDP | Discovery |

## How This Node Fits the Product

- Browser `Auto` mode keeps easy prompts local when possible.
- Harder prompts route to the verifier daemon.
- The daemon is designed to keep model weights hot while treating per-request state as disposable.
- Experimental browser scouts are benchmark-only and no longer part of the normal chat startup path.

## Next Steps

- [Architecture overview](architecture.md) for the local-first request flow
- [Deployment guide](deployment.md) for environment variables, HA setup, and monitoring
- [API contracts](api.md) for inference-mode headers and endpoints
- [Verification protocol](verification-protocol.md) for speculative validation when speculative mode is enabled
- [Remote Llama scout runbook](REMOTE_LLAMA_SCOUT_TEST_RUNBOOK.md) for the experimental WAN benchmark path
