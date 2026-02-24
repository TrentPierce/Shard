# Operations Runbook

## SLIs and SLOs
- Availability SLI: `% successful /v1/chat/completions` responses.
  - SLO: `99.9%` monthly.
- Latency SLI: `p95` end-to-end latency.
  - SLO: `p95 < 2500ms`.
- Speculative quality SLI: draft acceptance and reject rate.
  - SLO: acceptance `>= 0.60`, reject `<= 0.40`.
- Throughput SLI: tokens/sec and queue depth.
  - SLO: queue depth under steady-state threshold.

## Dashboards and Alerts
- Grafana dashboard: `deploy/monitoring/grafana/dashboards/shard-operations.json`
- Prometheus config: `deploy/monitoring/prometheus/prometheus.yml`
- Prometheus alert rules: `deploy/monitoring/prometheus/alerts.yml`
- Key alert triggers:
  - error-rate increase
  - latency regression
  - speculative acceptance collapse
  - PoW failure spikes

### Paging Thresholds
- `ShardHighTaskFailureRate`: page if task failures > 20/5m for 5m.
- `ShardHighP95Latency`: page if p95 latency > 2500ms for 10m.
- `ShardSpeculativeAcceptanceDrop`: warn if acceptance rate < 0.60 for 10m.
- `ShardPowFailureSpike`: warn if PoW failures > 30/10m.

### Alert-to-Runbook Mapping
- Ops alerts -> `docs/operations.md`
- Security alerts -> `docs/security.md`
- Model rollout controls -> `GET /v1/system/model-rollout`, `POST /v1/system/model-rollout/reset-rollback`

## Incident Handling
1. Confirm health and metrics endpoints.
2. Check auth/PoW failure counters.
3. Check browser contribution telemetry event stream (`shard:contribution-status`) and session state for non-contributing reason codes.
4. Drain failing nodes from scheduling path.
5. Roll back release if SLO breach persists.
6. Record timeline and corrective actions.

## Runbooks
- Incident response: `docs/runbooks/incident-response.md`
- On-call: `docs/runbooks/on-call.md`
- Latest tabletop outcome: `docs/runbooks/tabletop-2026-02-24.md`
- Benchmark proof protocol: `benchmarks/README.md`
