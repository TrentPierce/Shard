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

## Incident Handling
1. Confirm health and metrics endpoints.
2. Check auth/PoW failure counters.
3. Drain failing nodes from scheduling path.
4. Roll back release if SLO breach persists.
5. Record timeline and corrective actions.
