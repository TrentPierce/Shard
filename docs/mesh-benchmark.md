# Mesh Benchmark (Docker, Local)

This guide runs a local Shard mesh in Docker so you can compare throughput/latency at different node counts.

## What This Stack Runs

- `bootstrap` node exposed on `http://localhost:19091`
- `shard-node` workers (scale up/down with Docker Compose)

Both run with:
- `SHARD_REQUIRE_ENGINE_FOR_CONTRIBUTE=true` (real engine/model required for contribution)
- private bootstrap allowed for local bridge networking
- RC scheduler profile from `deploy/release/rc1.env`

## Model Requirement

The mesh compose stack expects the model at:

- `models/Llama-3.2-1B-Instruct-Q4_K_M.gguf`

The file is mounted read-only into containers as `/models/Llama-3.2-1B-Instruct-Q4_K_M.gguf`.

## Quick Start (Windows PowerShell)

From repo root:

```powershell
.\deploy\demo\mesh-up.ps1 -Nodes 1
```

Scale test:

```powershell
.\deploy\demo\mesh-up.ps1 -Nodes 2
.\deploy\demo\mesh-up.ps1 -Nodes 5
```

Stop and clean:

```powershell
docker compose -f deploy/demo/docker-compose.mesh.yml down -v
```

## Quick Start (bash)

```bash
./deploy/demo/mesh-up.sh 1
./deploy/demo/mesh-up.sh 2
./deploy/demo/mesh-up.sh 5
docker compose -f deploy/demo/docker-compose.mesh.yml down -v
```

## Health and Metrics

- Health: `http://localhost:19091/health`
- Metrics summary: `http://localhost:19091/metrics/summary`

Example:

```powershell
curl http://localhost:19091/metrics/summary
```

## Run Benchmark Harness

Create scenario files targeting the bootstrap API endpoint (`http://localhost:19091`), then run:

```powershell
python benchmarks/mesh_scale_benchmark.py `
  --scenarios-json benchmarks/scenarios.local.live.json `
  --out-dir benchmarks/results `
  --runs-per-scenario 3 `
  --requests-per-run 40 `
  --concurrency 8 `
  --inference-mode distributed `
  --collect-latency-breakdown
```

Compare node counts by repeating with different `-Nodes` values.
