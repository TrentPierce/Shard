# Run a Shard Node

This guide gets a verifier node running in under five minutes.

## Prerequisites

- Docker and Docker Compose
- A publicly reachable host (or port-forwarded home server)
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

## Verify It's Running

```bash
curl http://localhost:9091/health
```

A healthy response looks like:

```json
{ "status": "ok", "rust_version": "0.6.5" }
```

## Test Inference

```bash
curl http://localhost:9091/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{"model": "default", "messages": [{"role": "user", "content": "Hello"}]}'
```

## Model Provisioning Note

On first launch, ShardGUI auto-downloads the default verifier model from `deploy/models/manifest.json` and verifies its SHA-256 hash before enabling contribution. If the manifest endpoint is unavailable, ShardGUI falls back to a built-in default model URL. After download completes, ShardGUI restarts the node automatically and retries startup if the first restart does not come online.

## Required Ports

| Port | Protocol | Purpose |
|------|----------|---------|
| 4001 | TCP | libp2p mesh peering |
| 9091 | TCP | HTTP API / health |
| 9090 | UDP | QUIC transport |
| 9092 | UDP | Discovery |

## Next Steps

- [Architecture overview](architecture.md) — how Scouts and Verifiers interact
- [Deployment guide](deployment.md) — environment variables, HA setup, monitoring
- [Verification protocol](verification-protocol.md) — how draft tokens are validated
- [Mesh benchmark](mesh-benchmark.md) — compare throughput/latency with 1, 2, or more local Docker nodes

