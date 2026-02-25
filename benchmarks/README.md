# Benchmarking (Investor-Safe)

Use these tools for real, reproducible performance claims:

- `benchmarks/mesh_scale_benchmark.py`
- `benchmarks/run_pipeline_scale.py`

## Principles

- No simulated acceptance/speedup values.
- No inferred "best case" claims.
- Raw request-level data is exported for every run.
- Summary values include confidence intervals across repeated runs.
- Artifact manifest includes SHA256 hashes for integrity verification.

## Option A: Run Against Existing Environments

1. Create a scenarios file:

```json
{
  "scenarios": [
    { "name": "1-node", "node_count": 1, "base_url": "http://host-a:9091" },
    { "name": "3-node", "node_count": 3, "base_url": "http://host-b:9091" },
    { "name": "5-node", "node_count": 5, "base_url": "http://host-c:9091" }
  ]
}
```

2. Run benchmark:

```bash
python benchmarks/mesh_scale_benchmark.py \
  --scenarios-json benchmarks/scenarios.example.json \
  --runs-per-scenario 7 \
  --requests-per-run 60 \
  --concurrency 12 \
  --collect-latency-breakdown
```

## Option B: Automated Local 1/3/5 Node Pipeline

Prerequisite: Docker + Docker Compose.

```bash
python benchmarks/run_pipeline_scale.py \
  --runs-per-scenario 5 \
  --requests-per-run 40 \
  --concurrency 8
```

This script:
- Brings up 1, then 3, then 5-node topologies from `deploy/testnet/docker-compose.pipeline.yml`
- Waits for health readiness
- Runs the same benchmark protocol across all scenarios
- Tears down the stack at the end

## Output Artifacts

Each run writes to `benchmarks/results/mesh-scale-<timestamp>/`:

- `raw-runs.json` (all run-level and metrics snapshots)
- `raw-requests.csv` (request-level records)
- `summary.json` (scenario aggregates + CI)
- `report.md` (human-readable report)
- `metadata.json` (commit, params, seed)
- `manifest.json` (SHA256 for all artifacts)

## Interpreting Results

Key proof points:
- Throughput scaling by node count (`throughput_rps_mean`)
- Latency behavior (`latency_p95_ms_mean`)
- Latency breakdown (`ttft_avg_ms_mean`, `inter_token_avg_ms_mean`)
- Reliability (`success_rate_mean`)
- Speculative effectiveness (`measured_acceptance_rate_mean`, `measured_reject_rate_mean`)
- Failure causes (`error_distribution`)
- Transport stability (`transport_*_success_total_delta`, `transport_*_failure_total_delta`)

Only claim improvements that are visible in repeated-run means and confidence intervals.
